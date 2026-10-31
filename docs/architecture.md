# Architektur

## Übersicht

```
┌──────────────────────────── ECS (OMV, Docker) ───────────────────────────┐
│  autoweed-ecs  (Rust/axum + React)   InfluxDB        ntfy (selbst gehostet)│
│   - WebUI (Passwort)                  ▲               ▲                  │
│   - Konfiguration (Volume) ───────────┼───────────────┘                  │
│   - Polling-Job ──────────────────────┘                                  │
└───────────────┬──────────────────────────────────────────────────────────┘
                │ HTTP + Bearer-Token (LAN über Devolo)
┌───────────────▼────────── Board (Raspberry Pi 3, Docker) ────────────────┐
│  autoweed-board (Rust/axum + rppal)                                      │
│   - Regelschleife (Heizung, Pumpe, Licht) mit letzter Konfiguration      │
│   - Messwert-/Ereignis-Puffer                                            │
│   - Sensor-Treiber mit Back-off                                          │
└──────┬──────────────┬───────────────┬───────────────┬────────────────────┘
       │ I²C          │ I²C (ADC)     │ GPIO → Relais │ GPIO → MOSFET
   Temp/Feuchte   Bodenfeuchte     Heizung, Lampe    Pumpe 12 V
```

Nginx Proxy Manager macht nur die ECS-WebUI von außen erreichbar, das Board nicht.

## Verantwortlichkeiten

| | Board | ECS |
|---|---|---|
| Sensoren lesen | ✔ | |
| Aktoren schalten | ✔ | |
| Regelung (Hysterese, Lichtzyklus, Pumpenlimits) | ✔ eigenständig | |
| Konfiguration speichern | letzte Kopie | ✔ Hauptquelle, exportierbar |
| Messwerte speichern | Puffer, bis ECS sie abholt | ✔ InfluxDB |
| WebUI | | ✔ |
| Benachrichtigungen (ntfy) | meldet Fehler im Status | ✔ verschickt sie |
| Uhrzeit | NTP (öffentlicher Pool, systemd-timesyncd) | |

**Grundsatz:** Das Board kommt ohne ECS aus. ECS ist nur für Konfiguration, Anzeige und Historie zuständig.

## Board-API (Entwurf)

Alle Endpunkte erwarten `Authorization: Bearer <token>`. Der Token wird als Umgebungsvariable in beiden Portainer-Stacks gesetzt.

| Methode | Pfad | Zweck |
|---|---|---|
| `GET` | `/api/v1/status` | Aktuelle Messwerte, Aktor-Zustände, aktive Fehler, Konfigurationsversion, Zeit-Sync-Status |
| `GET` | `/api/v1/measurements?since=<cursor>` | Alle gepufferten Messwerte und Ereignisse seit `cursor` sowie den nächsten Cursor. Mit dem nächsten Abruf bestätigt ECS den Empfang, danach verwirft das Board die abgeholten Einträge. |
| `GET` | `/api/v1/config` | Konfiguration, die das Board gerade verwendet |
| `PUT` | `/api/v1/config` | Neue Konfiguration (komplett, versioniert). Das Board prüft sie und speichert sie lokal. |
| `POST` | `/api/v1/override/pump` | `{"seconds": n}`: einmal gießen. `n` ist durch das konfigurierte Maximum begrenzt. |
| `POST` | `/api/v1/override/{heater,light}` | `{"on": bool, "minutes": n}`: manuell übersteuern. Nach Ablauf übernimmt wieder die Regelung. |

Manuelle Befehle haben **immer eine Laufzeit**, ein dauerhaftes „an“ gibt es nicht. So bleibt kein Aktor hängen, wenn die Verbindung abbricht.

Die API-Typen liegen in einem gemeinsamen Crate (`shared`). Daraus werden mit `ts-rs` auch die TypeScript-Typen für das Frontend erzeugt.

## Regellogik

- **Heizung:** an, wenn `T < soll − h`; aus, wenn `T > soll + h`.
- **Pumpe:** Sie gießt, wenn `Feuchte < soll − h` gilt, der letzte Gießvorgang länger als der Mindestabstand her ist und die Tagesgrenze noch nicht erreicht ist. Dann läuft sie für die konfigurierte Laufzeit.
- **„Eimer leer“:** Steigt die Feuchte nach N aufeinanderfolgenden Gießvorgängen nicht um mindestens X an, wird die Pumpe gesperrt und ein Fehler gemeldet. Die Sperre hebt der Benutzer in der WebUI wieder auf.
- **Licht:** Der Zustand wird bei jedem Durchlauf aus der Uhrzeit berechnet: `an = (jetzt − startzeit) mod 24 h < X h`. Es muss kein Zyklus-Zustand gespeichert werden, nach einem Neustart stimmt der Stand sofort wieder.
- **Messintervall:** konfigurierbar zwischen 2 und 10 s (Standard 5 s). ECS fragt in einem eigenen Intervall ab, z. B. alle 10 s.

## Fail-Safe-Matrix

| Situation | Heizung | Licht | Pumpe | Meldung |
|---|---|---|---|---|
| Temperatursensor ausgefallen | aus | weiter nach Zeitplan | normal | UI, ntfy ab 10 s |
| Bodensensor ausgefallen | normal | weiter nach Zeitplan | gesperrt | UI, ntfy ab 10 s |
| Luftfeuchte ausgefallen | normal | normal | normal | UI, ntfy ab 10 s |
| ECS bzw. Netzwerk weg | normal | normal | normal | ECS: „Board nicht erreichbar“ (ntfy) |
| Internet weg (LAN ok) | normal | normal, die Uhr läuft weiter | normal | – |
| Board-Software abgestürzt | Docker-Neustart (`restart: always`), danach wieder normal | ← | ← | ECS: „Board nicht erreichbar“ |
| Board-Kernel hängt | Hardware-Watchdog des Pi startet ihn neu | ← | ← | ← |
| Board bekommt SIGTERM (z. B. Update) | – | – | sofort aus | – |
| Start ohne Zeit-Sync (Stromausfall und Internet weg) | normal | aus, bis die Zeit synchron ist | normal | UI, ntfy |
| ECS-Software hängt | Board regelt weiter | ← | ← | muss der Benutzer selbst bemerken |

Beim Start sind alle Aktoren aus, bis der erste Regeldurchlauf ihren Zustand festlegt.

**Back-off für Sensoren:** 1 → 2 → 5 → 10 → 30 → 60 s, danach alle 60 s. Ab der 10-s-Stufe verschickt ECS eine ntfy-Nachricht.

## Deployment

| Image | Plattform | Läuft auf | Zugriff auf Geräte |
|---|---|---|---|
| `ghcr.io/prinz-autistimus/autoweed-board` | linux/arm64 | Pi 3 (Raspberry Pi OS 64-bit) | `/dev/gpiochip0`, `/dev/i2c-1` |
| `ghcr.io/prinz-autistimus/autoweed-ecs` | linux/amd64 | ECS | – |

- Gebaut wird in GitHub Actions: das Board-Image nativ auf einem arm64-Runner (`ubuntu-24.04-arm`), das ECS-Image auf amd64. Der Entwickler-Laptop muss nichts bauen.
- Image-Tags: Push auf `main` → `:edge`; Git-Tag `v1.2.3` → `:1.2.3`, `:1.2`, `:latest` und ein GitHub-Release. Die Stacks verwenden standardmäßig `:latest`.
- Host-Port der WebUI auf ECS: **8097** (3000 ist belegt).
- Konfiguration und InfluxDB-Daten liegen in Docker-Volumes auf ECS.

### Compose-Dateien

Das Repo enthält zwei fertige Setups, die direkt als Portainer-Stack eingefügt werden können:

| Datei | Läuft auf | Inhalt |
|---|---|---|
| `deploy/pi-compose.yml` | Pi | `autoweed-board`, Watchtower |
| `deploy/host-compose.yml` | ECS | `autoweed-ecs` (Backend + WebUI), InfluxDB, Watchtower |

- Variable Werte (API-Token, WebUI-Passwort, Board-Adresse, InfluxDB-Zugang) kommen als Umgebungsvariablen aus Portainer. Eine `deploy/.env.example` dokumentiert sie.
- **ntfy** ist nicht Teil des Stacks. Benutzt wird die bestehende Instanz auf ECS: Stack `home_lax` (Fax), Host-Port 8090, öffentlich unter `https://ntfy.familie-nacken.de`. `autoweed-ecs` bekommt `NTFY_URL` und `NTFY_TOPIC` als Umgebungsvariablen. Der Stack hängt sich bewusst nicht in das Netzwerk `home_lax_fax-net`, damit er unabhängig vom Fax-Stack bleibt.
- ntfy läuft derzeit **ohne Authentifizierung**. Wer den Topic-Namen kennt, kann mitlesen und Nachrichten senden. Der Topic-Name muss deshalb zufällig sein (z. B. `autoweed-<zufall>`). Eine spätere Option sind ntfy-Access-Tokens, das würde aber den Fax-Stack ändern.
- Watchtower: Das Original `containrrr/watchtower` ist seit Dezember 2025 archiviert und hat Probleme mit Docker 29 (ECS läuft mit 29.4). Verwendet wird deshalb der gepflegte Fork `nicholas-fedor/watchtower`. Bisher läuft auf ECS noch kein Watchtower.
- Watchtower läuft in beiden Stacks mit `--label-enable` und aktualisiert **nur** die Container mit dem Label `com.centurylinklabs.watchtower.enable=true`. So fasst es die übrigen Container auf ECS nicht an.
- `autoweed-board` läuft mit `restart: always`. Es bekommt die Geräte `/dev/gpiochip0` und `/dev/i2c-1` und ein Volume für die lokale Konfigurationskopie.

### Portainer

- Auf dem Pi läuft der **Portainer Agent** (keine vollständige Portainer-Instanz). Er wird einmalig per `docker run` gestartet und ist im LAN auf Port 9001 erreichbar.
- Im ECS-Portainer wird der Pi als zusätzliche Umgebung („Environment → Agent“) eingebunden. Danach werden beide Stacks zentral aus dem ECS-Portainer verwaltet.
- Die Installationsschritte stehen später in `deploy/README.md`.

## Repo-Struktur (Vorschlag)

```
autoweed/
├── Cargo.toml          # Workspace
├── crates/
│   ├── shared/         # API-Typen, Konfigurationsschema
│   ├── board/          # Board-Software
│   └── ecs/            # ECS-Backend
├── frontend/           # React (Vite, TypeScript)
├── hardware/           # 3D-Modelle (STL + Quelldateien), Schaltplan
├── deploy/
│   ├── pi-compose.yml  # Stack für den Pi
│   ├── host-compose.yml# Stack für ECS
│   ├── .env.example
│   └── README.md       # Installation Pi + Portainer Agent
├── docs/
└── .github/workflows/
```
