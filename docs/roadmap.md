# Roadmap

Diese Datei ist der Arbeitsstand des Projekts. Sie wird nach jedem abgeschlossenen Schritt aktualisiert: Häkchen setzen und „Aktueller Stand“ anpassen.

Grundlagen: [vision.md](vision.md) (was), [architecture.md](architecture.md) (wie), [partslist.md](partslist.md) (womit).

## Aktueller Stand

- **Phase:** 4–6 umgesetzt, alle Teile bestellt. GPIO-Belegung festgelegt (GPIO5/6/13, Pins 29/31/33), Software-Review mit Fixes und ersten Unit-Tests erledigt (noch nicht released).
- **Nächster Schritt:** Server-Stack in Portainer neu deployen (Image heißt jetzt `autoweed-server`, Konfiguration vorher exportieren, siehe deploy/README.md), Pi einrichten (3.5, inkl. `config.txt`-Zeilen für die Boot-Pegel), 1-MΩ-Widerstand besorgen, nach Lieferung 4.3 auf echter Hardware testen.
- **Zuletzt aktualisiert:** 2026-11-01

## Offene Punkte

| Punkt | Benötigt in |
|---|---|
| Leistung der Grow-Lampe (mit Elektrotechniker) | Phase 7 |

---

## Phase 0: Planung ✅
- [x] Vision geklärt und überarbeitet
- [x] Architektur festgelegt: Board regelt eigenständig, Server für UI, Konfiguration und Historie
- [x] Fail-Safe-Matrix, API-Entwurf
- [x] Teileliste
- [x] Lizenz: AGPL-3.0
- [x] Bestehendes ntfy auf dem Server geprüft

## Phase 1: Repo und Gerüst
Repo: https://github.com/Prinz-Autistimus/autoweed

- [x] 1.1 `git init`, `.gitignore`, `README.md`, öffentliches GitHub-Repo anlegen
- [x] 1.2 Cargo-Workspace mit den Crates `shared`, `board`, `server` (je ein „Hello World“ mit axum)
- [x] 1.3 `frontend/` mit Vite, React und TypeScript
- [x] 1.4 `ts-rs`: TypeScript-Typen aus `shared` erzeugen
- [x] 1.5 **Teile bestellen** (bis 01.11.2026; nachträglich fehlt nur der 1-MΩ-Widerstand für A0)

## Phase 2: CI/CD
- [x] 2.1 GitHub Actions: `cargo check`/`clippy`, Frontend-Build
- [x] 2.2 Docker-Image `autoweed-board` (linux/arm64, nativer arm64-Runner) → GHCR
- [x] 2.3 Docker-Image `autoweed-server` (linux/amd64, mit Frontend) → GHCR
- [x] 2.4 Releases über Git-Tags (`v*`), Images mit Version + `latest` taggen

## Phase 3: Deployment-Gerüst
- [x] 3.1 `deploy/pi-compose.yml` (Board, Watchtower)
- [x] 3.2 `deploy/host-compose.yml` (Server, InfluxDB, Watchtower; ntfy extern)
- [x] 3.3 `deploy/.env.example`
- [x] 3.4 `deploy/README.md`: Pi OS 64-bit Lite (Pi 3), Docker, I²C aktivieren, Hardware-Watchdog, Portainer Agent, Einbindung in den Portainer des Servers
- [ ] 3.5 Hello-World-Stacks auf Pi und Server deployen und den Update-Weg über Watchtower testen
  - [x] Server: Stack `autoweed` läuft (Port 8097, InfluxDB ok, ntfy erreichbar, Watchtower nur auf AutoWeed-Container)
  - [ ] Pi einrichten und Stack deployen, `BOARD_URL` im Server-Stack auf die echte IP setzen
  - [x] Update über Watchtower beobachtet (v0.2.0)

## Phase 4: Board-Software
- [x] 4.1 Konfigurationsschema in `shared` (Temperatur, Bodenfeuchte, Pumpenlimits, Lichtmodi/Profile, Messintervall)
- [x] 4.2 Hardware-Abstraktion (Trait) mit **Simulations-Implementierung**, damit sich ohne Pi entwickeln lässt
- [ ] 4.3 Treiber: SHT31/SHT40, ADS1115 + Bodensensor, Relais, MOSFET (rppal) — geschrieben, **auf echter Hardware noch ungetestet**. Dabei prüfen: Relais ziehen beim Booten und Container-Start nicht an; Plausibilitätsgrenzen des Bodensensors (2000–25 000) passen zum echten Sensor mit 1 MΩ an A0
- [x] 4.4 Sensor-Back-off (1 → 2 → 5 → 10 → 30 → 60 s), Fehlerstatus
- [x] 4.5 Regelung Heizung (Hysterese)
- [x] 4.6 Regelung Licht (aus der Uhrzeit berechnet, Autoflower/Profile, Wechsel manuell oder nach X Tagen)
- [x] 4.7 Regelung Pumpe (Hysterese, Laufzeit, Mindestabstand, Tageslimit) und Erkennung „Eimer leer“
- [x] 4.8 Konfiguration lokal speichern, Start mit allen Aktoren aus, SIGTERM schaltet die Pumpe aus
- [x] 4.9 Messwert- und Ereignispuffer mit Cursor
- [x] 4.10 HTTP-API mit Bearer-Token (`status`, `measurements`, `config`, `override`)
- [x] 4.11 Fehler bei fehlendem Zeit-Sync (Licht bleibt aus, bis die Uhrzeit synchron ist)
- [x] 4.12 Review (01.11.2026): Pins ohne Schaltimpuls initialisieren, abgezogenen Bodensensor erkennen, `panic = "abort"`, Übersteuerung auf 24 h begrenzt, Ereignis-Zeitstempel eindeutig, Unit-Tests für Regelung, Licht, Puffer und Konfiguration

## Phase 5: Server-Backend
- [x] 5.1 Konfiguration speichern (Volume), versionieren, Export/Import
- [x] 5.2 Konfiguration an das Board senden, auch nach dessen Neustart
- [x] 5.3 Polling-Job: Messwerte und Ereignisse abholen → InfluxDB
- [x] 5.4 ntfy: Sensorfehler (ab 10 s), „Board nicht erreichbar“, „Eimer leer?“, fehlender Zeit-Sync
- [x] 5.5 Login mit Passwort (Session)
- [x] 5.6 REST-API für das Frontend, Frontend ausliefern

## Phase 6: Frontend
- [x] 6.1 Login
- [x] 6.2 Dashboard: aktuelle Werte, Aktor-Zustände, Fehler
- [x] 6.3 Konfiguration: Temperatur, Bodenfeuchte, Pumpe, Licht (Modus, Profile)
- [x] 6.4 Diagramme über einen wählbaren Zeitraum und eine Ereignisliste
- [x] 6.5 Manuelle Übersteuerung (Pumpe einmal gießen, Licht/Heizung für X Minuten)
- [x] 6.6 Konfiguration exportieren/importieren
- [ ] 6.7 Mobile Ansicht — Layout responsiv gebaut, auf dem Handy noch nicht geprüft

## Phase 7: Hardware
- [x] 7.1 Schaltplan und GPIO-Belegung — Verdrahtungspläne in `hardware/wiring/` (WireViz, gerendert im Release); Belegung final: GPIO5 Heizung, GPIO6 Lampe, GPIO13 Pumpe, 1 MΩ A0 → GND
- [ ] 7.2 Kleinspannungsseite aufbauen und testen (Pi, ADS1115, SHT, MOSFET, Relais ohne Last)
- [ ] 7.3 230-V-Seite durch den Elektrotechniker (Netzteil, Relais, Schuko, C14)
- [ ] 7.4 Gehäuse konstruieren und in PETG drucken: Controller (GX12, USB, RJ45 in der Wand) und Sensorgehäuse
- [ ] 7.5 Kabel mit GX12-Steckern konfektionieren

## Phase 8: Inbetriebnahme
- [ ] 8.1 Bodensensor kalibrieren (trocken/nass), mit eingebautem 1-MΩ-Widerstand
- [ ] 8.2 Probelauf ohne Pflanze (mehrere Tage)
- [ ] 8.3 Fail-Safe-Tests nach der Matrix: Sensor abziehen, Container killen, LAN trennen, Server stoppen, Strom weg
- [ ] 8.4 Proxy-Host im NPM (erledigt), ntfy-Topic (zufälliger Name) in der App abonnieren
- [ ] 8.5 Erster Grow 🌱

## Später
- [ ] Kamera über USB
- [ ] ntfy-Access-Tokens
