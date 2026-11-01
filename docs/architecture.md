# Architektur

## Übersicht

```
┌─────────────────────────── Server (OMV, Docker) ─────────────────────────┐
│  autoweed-server (Rust/axum + React) InfluxDB        ntfy (selbst gehostet)│
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

Nginx Proxy Manager macht nur die WebUI des Servers von außen erreichbar, das Board nicht.

## Verantwortlichkeiten

| | Board | Server |
|---|---|---|
| Sensoren lesen | ✔ | |
| Aktoren schalten | ✔ | |
| Regelung (Hysterese, Lichtzyklus, Pumpenlimits) | ✔ eigenständig | |
| Konfiguration speichern | letzte Kopie | ✔ Hauptquelle, exportierbar |
| Messwerte speichern | Puffer, bis der Server sie abholt | ✔ InfluxDB |
| WebUI | | ✔ |
| Benachrichtigungen (ntfy) | meldet Fehler im Status | ✔ verschickt sie |
| Uhrzeit | NTP (öffentlicher Pool, systemd-timesyncd) | |

**Grundsatz:** Das Board kommt ohne den Server aus. Der Server ist nur für Konfiguration, Anzeige und Historie zuständig.

## Board-API (Entwurf)

Alle Endpunkte erwarten `Authorization: Bearer <token>`. Der Token wird als Umgebungsvariable in beiden Portainer-Stacks gesetzt.

| Methode | Pfad | Zweck |
|---|---|---|
| `GET` | `/api/v1/status` | Aktuelle Messwerte, Aktor-Zustände, aktive Fehler, Konfigurationsversion, Zeit-Sync-Status |
| `GET` | `/api/v1/measurements?since=<cursor>` | Alle gepufferten Messwerte und Ereignisse seit `cursor` sowie den nächsten Cursor. Mit dem nächsten Abruf bestätigt der Server den Empfang, danach verwirft das Board die abgeholten Einträge. |
| `GET` | `/api/v1/config` | Konfiguration, die das Board gerade verwendet |
| `PUT` | `/api/v1/config` | Neue Konfiguration (komplett, versioniert). Das Board prüft sie und speichert sie lokal. |
| `POST` | `/api/v1/override/pump` | `{"secs": n}`: einmal gießen, höchstens 120 s. |
| `POST` | `/api/v1/override/{heater,light}` | `{"on": bool, "minutes": n}`: manuell übersteuern, `minutes: 0` beendet die Übersteuerung. Nach Ablauf übernimmt wieder die Regelung. |
| `POST` | `/api/v1/pump/unlock` | Sperre „Eimer leer“ aufheben |

Manuelle Befehle haben **immer eine Laufzeit** (Pumpe höchstens 120 s, Heizung/Licht höchstens 24 h), ein dauerhaftes „an“ gibt es nicht. So bleibt kein Aktor hängen, wenn die Verbindung abbricht.

Die API-Typen liegen in einem gemeinsamen Crate (`shared`). Daraus werden mit `ts-rs` auch die TypeScript-Typen für das Frontend erzeugt.

## Pinbelegung (BCM)

| Funktion | GPIO | Pin | Zustand beim Booten |
|---|---|---|---|
| 3,3 V → Verteiler (SHT40, ADS1115, Bodensensor) | – | 1 | |
| I²C SDA (SHT40, ADS1115) | 2 | 3 | |
| I²C SCL | 3 | 5 | |
| GND → Verteiler | – | 6 | |
| 3,3 V → Relais-VCC (Optokoppler) | – | 17 | |
| Relais K1 Heizung (IN1) | 5 | 29 | Pull-up → Relais aus |
| Relais K2 Lampe (IN2) | 6 | 31 | Pull-up → Relais aus |
| MOSFET Pumpe (TRIG) | 13 | 33 | Pull-down → Pumpe aus |
| GND → MOSFET | – | 34 | |

Warum diese Pins:

- **Sicherer Zustand beim Einschalten.** Bis der Container läuft, vergehen 30–60 s, in denen die Software nichts schaltet. Das Relais-Modul schaltet bei LOW. GPIO 0–8 haben nach dem Reset einen Pull-up, GPIO 9–27 einen Pull-down. Die Relais hängen deshalb an GPIO5/6 (HIGH = aus), die Pumpe an GPIO13 (LOW = aus). Mit der alten Belegung (GPIO17/27) zogen die Pull-downs die Relais-Eingänge auf LOW, also auf „an“. Zusätzlich setzt die Firmware die Pegel per `config.txt` (siehe `deploy/README.md`).
- **Keine Doppelbelegung:** GPIO5/6/13 gehören zu keiner Schnittstelle, die später gebraucht werden könnte (I²C, SPI, UART, 1-Wire auf GPIO4, HAT-EEPROM auf GPIO0/1).
- **Ein Stecker:** Pins 29, 31, 33 und 34 liegen nebeneinander am unteren Ende der Leiste.
- GPIO13 kann Hardware-PWM, falls die Pumpe später gedrosselt werden soll.

Die Ausgänge sind über `HEATER_PIN`, `LIGHT_PIN` und `PUMP_PIN` im Pi-Stack änderbar. Die verbindliche Verdrahtung steht in `hardware/wiring/`.

**Bodensensor-Plausibilität:** Zwischen A0 des ADS1115 und GND sitzt ein 1-MΩ-Widerstand. Ist der Sensor abgezogen, liest der ADC dann sicher fast 0 statt eines zufälligen Werts. Das Board wertet Rohwerte unter 2000 oder über 25 000 als Sensorfehler, gesperrt wird dann wie bei einem I²C-Fehler. Ohne diese Prüfung könnte ein offener Eingang „trocken“ melden und die Pumpe auslösen.

## Regellogik

- **Heizung:** an, wenn `T < soll − h`; aus, wenn `T > soll + h`.
- **Pumpe:** Sie gießt, wenn `Feuchte < soll − h` gilt, der letzte Gießvorgang länger als der Mindestabstand her ist und die Tagesgrenze noch nicht erreicht ist. Dann läuft sie für die konfigurierte Laufzeit.
- **„Eimer leer“:** Steigt die Feuchte nach N aufeinanderfolgenden Gießvorgängen nicht um mindestens X an, wird die Pumpe gesperrt und ein Fehler gemeldet. Die Sperre hebt der Benutzer in der WebUI wieder auf.
- **Licht:** Der Zustand wird bei jedem Durchlauf aus der Uhrzeit berechnet: `an = (jetzt − startzeit) mod 24 h < X h`. Es muss kein Zyklus-Zustand gespeichert werden, nach einem Neustart stimmt der Stand sofort wieder.
- **Messintervall:** konfigurierbar zwischen 2 und 10 s (Standard 5 s). Der Server fragt in einem eigenen Intervall ab, z. B. alle 10 s.

## Fail-Safe-Matrix

| Situation | Heizung | Licht | Pumpe | Meldung |
|---|---|---|---|---|
| Temperatursensor ausgefallen | aus | weiter nach Zeitplan | normal | UI, ntfy ab 10 s |
| Bodensensor ausgefallen oder abgezogen (Rohwert unplausibel) | normal | weiter nach Zeitplan | gesperrt | UI, ntfy ab 10 s |
| Luftfeuchte ausgefallen | normal | normal | normal | UI, ntfy ab 10 s |
| Server bzw. Netzwerk weg | normal | normal | normal | Server: „Board nicht erreichbar“ (ntfy) |
| Internet weg (LAN ok) | normal | normal, die Uhr läuft weiter | normal | – |
| Board-Software abgestürzt (auch Panic in der Regelschleife: `panic = "abort"` beendet den ganzen Prozess) | Docker-Neustart (`restart: always`), danach wieder normal | ← | ← | Server: „Board nicht erreichbar“ |
| Board-Kernel hängt | Hardware-Watchdog des Pi startet ihn neu | ← | ← | ← |
| Board bekommt SIGTERM (z. B. Update) | – | – | sofort aus | – |
| Start ohne Zeit-Sync (Stromausfall und Internet weg) | normal | aus, bis die Zeit synchron ist | normal | UI, ntfy |
| Server-Software hängt | Board regelt weiter | ← | ← | muss der Benutzer selbst bemerken |

Beim Start sind alle Aktoren aus, bis der erste Regeldurchlauf ihren Zustand festlegt. Die Software setzt den „aus“-Pegel, bevor sie einen Pin auf Ausgang schaltet, damit kein Relais kurz anzieht.

**Back-off für Sensoren:** 1 → 2 → 5 → 10 → 30 → 60 s, danach alle 60 s. Ab der 10-s-Stufe verschickt der Server eine ntfy-Nachricht.

## Deployment

| Image | Plattform | Läuft auf | Zugriff auf Geräte |
|---|---|---|---|
| `ghcr.io/prinz-autistimus/autoweed-board` | linux/arm64 | Pi 3 (Raspberry Pi OS 64-bit) | `/dev/gpiomem`, `/dev/i2c-1` |
| `ghcr.io/prinz-autistimus/autoweed-server` | linux/amd64 | Server | – |

- Gebaut wird in GitHub Actions: das Board-Image nativ auf einem arm64-Runner (`ubuntu-24.04-arm`), das Server-Image auf amd64. Der Entwickler-Laptop muss nichts bauen.
- Image-Tags: Push auf `main` → `:edge`; Git-Tag `v1.2.3` → `:1.2.3`, `:1.2`, `:latest` und ein GitHub-Release. Die Stacks verwenden standardmäßig `:latest`.
- Host-Port der WebUI auf dem Server: **8097** (3000 ist belegt).
- Konfiguration und InfluxDB-Daten liegen in Docker-Volumes auf dem Server.

### Compose-Dateien

Das Repo enthält zwei fertige Setups, die direkt als Portainer-Stack eingefügt werden können:

| Datei | Läuft auf | Inhalt |
|---|---|---|
| `deploy/pi-compose.yml` | Pi | `autoweed-board`, Watchtower |
| `deploy/host-compose.yml` | Server | `autoweed-server` (Backend + WebUI), InfluxDB, Watchtower |

- Variable Werte (API-Token, WebUI-Passwort, Board-Adresse, InfluxDB-Zugang) kommen als Umgebungsvariablen aus Portainer. Eine `deploy/.env.example` dokumentiert sie.
- **ntfy** ist nicht Teil des Stacks. Benutzt wird eine bereits vorhandene, selbst gehostete ntfy-Instanz. `autoweed-server` bekommt `NTFY_URL` und `NTFY_TOPIC` als Umgebungsvariablen und erreicht ntfy standardmäßig über dessen Host-Port (`host.docker.internal:8090`).
- Der Topic-Name sollte zufällig gewählt werden (z. B. `autoweed-<zufall>`), sofern die ntfy-Instanz keine Zugriffskontrolle nutzt.
- Watchtower: Das Original `containrrr/watchtower` ist seit Dezember 2025 archiviert und hat Probleme mit Docker 29 (der Server läuft mit 29.4). Verwendet wird deshalb der gepflegte Fork `nicholas-fedor/watchtower`. Bisher läuft auf dem Server noch kein Watchtower.
- Watchtower läuft in beiden Stacks mit `--label-enable` und aktualisiert **nur** die Container mit dem Label `com.centurylinklabs.watchtower.enable=true`. So fasst es die übrigen Container auf dem Server nicht an.
- `autoweed-board` läuft mit `restart: always`. Es bekommt die Geräte `/dev/gpiomem` und `/dev/i2c-1` und ein Volume für die lokale Konfigurationskopie.

### Portainer

- Auf dem Pi läuft der **Portainer Agent** (keine vollständige Portainer-Instanz). Er wird einmalig per `docker run` gestartet und ist im LAN auf Port 9001 erreichbar.
- Im Portainer des Servers wird der Pi als zusätzliche Umgebung („Environment → Agent“) eingebunden. Danach werden beide Stacks zentral aus dem Portainer des Servers verwaltet.
- Die Installationsschritte stehen später in `deploy/README.md`.

## Repo-Struktur (Vorschlag)

```
autoweed/
├── Cargo.toml          # Workspace
├── crates/
│   ├── shared/         # API-Typen, Konfigurationsschema
│   ├── board/          # Board-Software
│   └── server/         # Server (Backend + WebUI-Auslieferung)
├── frontend/           # React (Vite, TypeScript)
├── hardware/           # 3D-Modelle (STL + Quelldateien), Schaltplan
├── deploy/
│   ├── pi-compose.yml  # Stack für den Pi
│   ├── host-compose.yml# Stack für den Server
│   ├── .env.example
│   └── README.md       # Installation Pi + Portainer Agent
├── docs/
└── .github/workflows/
```
