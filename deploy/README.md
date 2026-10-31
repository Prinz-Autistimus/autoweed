# Deployment

| Datei | Stack für | Inhalt |
|---|---|---|
| [pi-compose.yml](pi-compose.yml) | Raspberry Pi 3 (Board) | `autoweed-board`, Watchtower |
| [host-compose.yml](host-compose.yml) | ECS | `autoweed-ecs`, InfluxDB, Watchtower |
| [.env.example](.env.example) | beide | Variablen, die in Portainer gesetzt werden |

Beide Stacks werden aus dem Portainer auf ECS verwaltet. Auf dem Pi läuft dafür nur der Portainer Agent.

## 1. Raspberry Pi vorbereiten

1. Mit dem **Raspberry Pi Imager** „Raspberry Pi OS Lite (64-bit)“ auf die microSD schreiben. In den Einstellungen des Imagers:
   - Hostname `autoweed`
   - Benutzer und Passwort
   - SSH aktivieren (am besten mit Public Key)
   - WLAN leer lassen: Der Pi hängt per LAN am Devolo.
2. Im Router für den Pi eine **feste IP** reservieren (DHCP-Reservierung). Sie wird für `BOARD_URL` und den Agent gebraucht.
3. Per SSH einloggen und das System aktualisieren:
   ```bash
   sudo apt update && sudo apt full-upgrade -y
   ```
4. **I²C aktivieren** (für Temperatursensor und ADS1115):
   ```bash
   sudo raspi-config nonint do_i2c 0
   sudo reboot
   ls /dev/i2c-1 /dev/gpiomem      # beide müssen existieren
   ls /run/systemd/timesync/synchronized   # Uhr synchron (sonst bleibt das Licht aus)
   ```
5. **Hardware-Watchdog** einschalten. Bleibt der Kernel hängen, startet der Pi nach 15 s neu (15 s ist das Maximum beim Pi):
   ```bash
   sudo mkdir -p /etc/systemd/system.conf.d
   printf '[Manager]\nRuntimeWatchdogSec=15s\nRebootWatchdogSec=2min\n' | sudo tee /etc/systemd/system.conf.d/watchdog.conf
   sudo reboot
   ```
6. **Docker installieren:**
   ```bash
   curl -fsSL https://get.docker.com | sudo sh
   sudo usermod -aG docker "$USER"   # danach neu einloggen
   ```

## 2. Portainer Agent auf dem Pi

Die Version muss zum Portainer auf ECS passen (derzeit **2.33.7**, nachsehen unter Portainer → Einstellungen bzw. unten links).

```bash
docker run -d --name portainer_agent --restart=always \
  -p 9001:9001 \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v /var/lib/docker/volumes:/var/lib/docker/volumes \
  -v /:/host \
  portainer/agent:2.33.7
```

Dann im **ECS-Portainer**: *Environments → Add environment → Docker Standalone → Agent*
- Name: `autoweed-pi`
- Environment address: `<pi-ip>:9001`

> Der Agent gibt vollen Zugriff auf Docker auf dem Pi. Er bindet sich an den ersten Portainer-Server, der sich verbindet. Deshalb den Pi direkt nach dem Start des Agents einbinden und Port 9001 nicht über den Router freigeben.

Wird Portainer auf ECS aktualisiert, muss der Agent dieselbe Version bekommen: den Container löschen und den Befehl oben mit der neuen Version erneut ausführen.

## 3. Stacks anlegen

Die Werte für die Variablen erzeugt man mit `openssl rand -hex 32`. `BOARD_TOKEN` muss in beiden Stacks **gleich** sein.

### Pi (Umgebung `autoweed-pi`)

*Stacks → Add stack → Repository*
- Repository URL: `https://github.com/Prinz-Autistimus/autoweed`
- Repository reference: `refs/heads/main`
- Compose path: `deploy/pi-compose.yml`
- Environment variables: `BOARD_TOKEN` (optional `CLIMATE_SENSOR`, `HEATER_PIN`, `LIGHT_PIN`, `PUMP_PIN`, `RELAY_ACTIVE_LOW`, `AUTOWEED_TAG`, `WATCHTOWER_INTERVAL`)

### ECS (Umgebung `local`)

*Stacks → Add stack → Repository*, wie oben, aber:
- Compose path: `deploy/host-compose.yml`
- Environment variables: `BOARD_TOKEN`, `BOARD_URL`, `WEBUI_PASSWORD`, `NTFY_TOPIC`, `INFLUX_PASSWORD`, `INFLUX_TOKEN` (optional `PUBLIC_URL`, `AUTOWEED_PORT`, `NTFY_URL`, `POLL_INTERVAL_SECS`, `AUTOWEED_TAG`). `WEBUI_PASSWORD` braucht mindestens 8 Zeichen.

Die WebUI ist dann unter `http://<ecs-ip>:8097` erreichbar. Für den Zugriff von außen wird im Nginx Proxy Manager ein Proxy Host auf diesen Port angelegt.

## 4. Lokal entwickeln (ohne Pi)

```bash
# Board als Simulation (SIM_SPEED beschleunigt die Zeit, SIM_FAIL_RATE lässt Messungen fehlschlagen)
HARDWARE=sim SIM_SPEED=30 TIME_SYNC_FILE= BOARD_TOKEN=devtoken-1234567890 DATA_DIR=/tmp/aw-board \
  BIND_ADDR=127.0.0.1:8080 cargo run -p autoweed-board

# ECS gegen das simulierte Board (braucht eine InfluxDB 2 auf :8086)
WEBUI_PASSWORD=devpasswort BOARD_URL=http://127.0.0.1:8080 BOARD_TOKEN=devtoken-1234567890 \
  INFLUX_URL=http://127.0.0.1:8086 INFLUX_TOKEN=... NTFY_URL=http://127.0.0.1:9999 NTFY_TOPIC=test \
  DATA_DIR=/tmp/aw-ecs BIND_ADDR=127.0.0.1:3000 cargo run -p autoweed-ecs

cd frontend && npm run dev     # oder npm run build, dann liefert ECS die WebUI aus
```

## 5. Updates

```
git tag v0.2.0 && git push --tags
   → GitHub Actions baut die Images → GHCR (:0.2.0, :0.2, :latest) + GitHub-Release
   → Watchtower auf Pi und ECS holt das neue Image (innerhalb von WATCHTOWER_INTERVAL)
```

- `AUTOWEED_TAG=latest` (Standard): nur Releases.
- `AUTOWEED_TAG=edge`: jeder Push auf `main`, gedacht zum Testen.

Ändern sich die Compose-Dateien selbst, wird der Stack in Portainer neu deployt („Pull and redeploy“). Watchtower aktualisiert nur Images.
