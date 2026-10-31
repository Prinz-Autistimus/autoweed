# Roadmap

Diese Datei ist der Arbeitsstand des Projekts. Sie wird nach jedem abgeschlossenen Schritt aktualisiert: Häkchen setzen und „Aktueller Stand“ anpassen.

Grundlagen: [vision.md](vision.md) (was), [architecture.md](architecture.md) (wie), [partslist.md](partslist.md) (womit).

## Aktueller Stand

- **Phase:** 4 (parallel: Pi-Einrichtung 3.5 und Teilebestellung 1.5 durch den Benutzer).
- **Nächster Schritt:** 4.2 Hardware-Abstraktion mit Simulation.
- **Zuletzt aktualisiert:** 2026-10-31

## Offene Punkte

| Punkt | Benötigt in |
|---|---|
| Leistung der Grow-Lampe (mit Elektrotechniker) | Phase 7 |
| Domain/Subdomain für die WebUI (z. B. `autoweed.familie-nacken.de`) | Phase 8 |
| GPIO-Belegung | Phase 4 / 7 |

---

## Phase 0: Planung ✅
- [x] Vision geklärt und überarbeitet
- [x] Architektur festgelegt: Board regelt eigenständig, ECS für UI, Konfiguration und Historie
- [x] Fail-Safe-Matrix, API-Entwurf
- [x] Teileliste
- [x] Lizenz: AGPL-3.0
- [x] Bestehendes ntfy auf ECS geprüft

## Phase 1: Repo und Gerüst
Repo: https://github.com/Prinz-Autistimus/autoweed

- [x] 1.1 `git init`, `.gitignore`, `README.md`, öffentliches GitHub-Repo anlegen
- [x] 1.2 Cargo-Workspace mit den Crates `shared`, `board`, `ecs` (je ein „Hello World“ mit axum)
- [x] 1.3 `frontend/` mit Vite, React und TypeScript
- [x] 1.4 `ts-rs`: TypeScript-Typen aus `shared` erzeugen
- [ ] 1.5 **Teile bestellen**: AliExpress braucht Wochen, deshalb parallel zur Software

## Phase 2: CI/CD
- [x] 2.1 GitHub Actions: `cargo check`/`clippy`, Frontend-Build
- [x] 2.2 Docker-Image `autoweed-board` (linux/arm64, nativer arm64-Runner) → GHCR
- [x] 2.3 Docker-Image `autoweed-ecs` (linux/amd64, mit Frontend) → GHCR
- [x] 2.4 Releases über Git-Tags (`v*`), Images mit Version + `latest` taggen

## Phase 3: Deployment-Gerüst
- [x] 3.1 `deploy/pi-compose.yml` (Board, Watchtower)
- [x] 3.2 `deploy/host-compose.yml` (ECS, InfluxDB, Watchtower; ntfy extern)
- [x] 3.3 `deploy/.env.example`
- [x] 3.4 `deploy/README.md`: Pi OS 64-bit Lite (Pi 3), Docker, I²C aktivieren, Hardware-Watchdog, Portainer Agent, Einbindung in den ECS-Portainer
- [ ] 3.5 Hello-World-Stacks auf Pi und ECS deployen und den Update-Weg über Watchtower testen
  - [x] ECS: Stack `autoweed` läuft (Port 8097, InfluxDB ok, ntfy erreichbar, Watchtower nur auf AutoWeed-Container)
  - [ ] Pi einrichten und Stack deployen, `BOARD_URL` im ECS-Stack auf die echte IP setzen
  - [ ] Update über Watchtower beim nächsten Release beobachten

## Phase 4: Board-Software
- [x] 4.1 Konfigurationsschema in `shared` (Temperatur, Bodenfeuchte, Pumpenlimits, Lichtmodi/Profile, Messintervall)
- [ ] 4.2 Hardware-Abstraktion (Trait) mit **Simulations-Implementierung**, damit sich ohne Pi entwickeln lässt
- [ ] 4.3 Treiber: SHT31/SHT40, ADS1115 + Bodensensor, Relais, MOSFET (rppal)
- [ ] 4.4 Sensor-Back-off (1 → 2 → 5 → 10 → 30 → 60 s), Fehlerstatus
- [ ] 4.5 Regelung Heizung (Hysterese)
- [ ] 4.6 Regelung Licht (aus der Uhrzeit berechnet, Autoflower/Profile, Wechsel manuell oder nach X Tagen)
- [ ] 4.7 Regelung Pumpe (Hysterese, Laufzeit, Mindestabstand, Tageslimit) und Erkennung „Eimer leer“
- [ ] 4.8 Konfiguration lokal speichern, Start mit allen Aktoren aus, SIGTERM schaltet die Pumpe aus
- [ ] 4.9 Messwert- und Ereignispuffer mit Cursor
- [ ] 4.10 HTTP-API mit Bearer-Token (`status`, `measurements`, `config`, `override`)
- [ ] 4.11 Fehler bei fehlendem Zeit-Sync (Licht bleibt aus, bis die Uhrzeit synchron ist)

## Phase 5: ECS-Backend
- [ ] 5.1 Konfiguration speichern (Volume), versionieren, Export/Import
- [ ] 5.2 Konfiguration an das Board senden, auch nach dessen Neustart
- [ ] 5.3 Polling-Job: Messwerte und Ereignisse abholen → InfluxDB
- [ ] 5.4 ntfy: Sensorfehler (ab 10 s), „Board nicht erreichbar“, „Eimer leer?“, fehlender Zeit-Sync
- [ ] 5.5 Login mit Passwort (Session)
- [ ] 5.6 REST-API für das Frontend, Frontend ausliefern

## Phase 6: Frontend
- [ ] 6.1 Login
- [ ] 6.2 Dashboard: aktuelle Werte, Aktor-Zustände, Fehler
- [ ] 6.3 Konfiguration: Temperatur, Bodenfeuchte, Pumpe, Licht (Modus, Profile)
- [ ] 6.4 Diagramme über einen wählbaren Zeitraum und eine Ereignisliste
- [ ] 6.5 Manuelle Übersteuerung (Pumpe einmal gießen, Licht/Heizung für X Minuten)
- [ ] 6.6 Konfiguration exportieren/importieren
- [ ] 6.7 Mobile Ansicht

## Phase 7: Hardware
- [ ] 7.1 Schaltplan und GPIO-Belegung
- [ ] 7.2 Kleinspannungsseite aufbauen und testen (Pi, ADS1115, SHT, MOSFET, Relais ohne Last)
- [ ] 7.3 230-V-Seite durch den Elektrotechniker (Netzteil, Relais, Schuko, C14)
- [ ] 7.4 Gehäuse konstruieren und in PETG drucken: Controller (GX12, USB, RJ45 in der Wand) und Sensorgehäuse
- [ ] 7.5 Kabel mit GX12-Steckern konfektionieren

## Phase 8: Inbetriebnahme
- [ ] 8.1 Bodensensor kalibrieren (trocken/nass)
- [ ] 8.2 Probelauf ohne Pflanze (mehrere Tage)
- [ ] 8.3 Fail-Safe-Tests nach der Matrix: Sensor abziehen, Container killen, LAN trennen, ECS stoppen, Strom weg
- [ ] 8.4 Proxy-Host im NPM, ntfy-Topic (zufälliger Name) in der App abonnieren
- [ ] 8.5 Erster Grow 🌱

## Später
- [ ] Kamera über USB
- [ ] ntfy-Access-Tokens (betrifft den Fax-Stack)
