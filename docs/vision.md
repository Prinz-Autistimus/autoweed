# Auto Weed

## Einleitung

Eine Cannabis-Pflanze bis zur Ernte großzuziehen kann unfassbar schwierig sein, da die Pflanzen viel Aufmerksamkeit brauchen.
Hierfür ist dieses Projekt gedacht. Es ist ein Open-Source-Projekt (AGPL-3.0), alle Dateien (Code, 3D-Modelle, Doku) werden auf GitHub veröffentlicht.

Ausgelegt ist das System für **eine Pflanze** in **einem Grow-Zelt** (bis 1,8 m Höhe). Das Zelt steht in einer kühlen Garage.

## Legende
- **Board-Computer / Board:** Raspberry Pi 3 (vorhanden) am Zelt. Liest die Sensoren, schaltet die Aktoren und regelt selbstständig.
- **Server:** Der Home-Server (OpenMediaVault + Docker). Hier laufen WebUI, Konfiguration, Datenbank und Benachrichtigungen.
- **(Sub-)Modul:** Sensoren und Aktoren, per Kabel mit dem Board verbunden.

## Aufbau

Das System besteht aus zwei Programmen (Details in [architecture.md](architecture.md)):

1. **Board-Software** (Rust, auf dem Raspberry Pi): Sie bietet eine HTTP-API, liest die Sensoren und schaltet die Aktoren. Mit der zuletzt vom Server erhaltenen Konfiguration regelt sie **eigenständig** weiter, auch wenn der Server oder das Netzwerk ausfallen.
2. **Server-Software** (Rust-Backend + React-Frontend, auf dem Server): Sie stellt die WebUI bereit, verwaltet die Konfiguration, holt periodisch die Messwerte vom Board ab, speichert sie in InfluxDB und verschickt Benachrichtigungen über ntfy.

Das Board liegt oben auf dem Zelt und ist per LAN (über Devolo) mit dem Heimnetz verbunden.

### Aktoren
- **Relais-Board (10 A, 230 V):** Schaltet die E-Heizung und die Grow-Lampe. Die Lampe hat keine eigene Smart-Steuerung. Die Heizung dient dazu, die Temperatur konstant zu halten. Bei Übertemperatur greift die Software nicht ein, dafür gibt es den in der Heizung eingebauten Überhitzungsschutz.
- **Wasserpumpe (12 V, Tauchpumpe):** Sie gießt die Pflanze aus einem Eimer mit fertig gemischter Wasser-Dünger-Lösung, es gibt keinen Anschluss ans Wassernetz. Eimer und Topf stehen auf gleicher Höhe, der Eimer wird manuell befüllt.

### Sensoren
- **Temperatur- und Luftfeuchtigkeitssensor** (kombiniert, I²C, hängt oben im Zelt): Die Temperatur steuert die Heizung. Die Luftfeuchtigkeit wird **nur überwacht** und nicht geregelt.
- **Bodenfeuchtigkeitssensor** (kapazitiv, über ADC ADS1115, in der Erde): Er überwacht, ob die Erde genug gewässert ist, und steuert die Pumpe.

Der Luftfilter mit Lüfter ist vorhanden, läuft aber unabhängig und wird nicht vom System gesteuert.

## Integration
Weichen die Messwerte der Sensoren von den eingestellten Zielwerten ab, reagieren die Aktoren:
- Liegt die Temperatur unter Sollwert minus Hysterese, wird die **Heizung** eingeschaltet. Liegt sie über Sollwert plus Hysterese, wird sie ausgeschaltet.
- Liegt die Bodenfeuchtigkeit unter Sollwert minus Hysterese, gießt die **Pumpe** eine Portion. Danach wartet sie, bis das Wasser eingezogen ist.
- Die **Lampe** folgt dem eingestellten Lichtzyklus.

## Interaktion
Der Benutzer konfiguriert alle Wachstumsparameter über eine WebUI. Sie ist für Mobilgeräte geeignet, mit einem einfachen Passwort geschützt und auch von außerhalb erreichbar (über den Nginx Proxy Manager). Einstellbar sind:
- Zieltemperatur mit Hysterese
- Ziel-Bodenfeuchtigkeit mit Hysterese
- Pumpen-Grenzwerte: Laufzeit pro Gießvorgang, Mindestabstand zwischen zwei Gießvorgängen, maximale Laufzeit pro Tag
- Lichtzyklus (siehe Features)

Außerdem kann der Benutzer über die WebUI die Messdaten und Ereignisse (z. B. „Pumpe lief 10 s“, „Heizung an“) eines frei wählbaren Zeitraums ansehen. So lässt sich nachvollziehen, ob das gesamte System korrekt arbeitet. Die Konfiguration ist exportierbar.

## Umsetzung/Anforderungen
- **Entwicklung und Release:** Entwickelt wird mit Git, GitHub ist das (öffentliche) Remote. GitHub Actions baut die Docker-Images (Board: arm64) und veröffentlicht sie in der GitHub Container Registry (GHCR).
- **Updates:** Watchtower holt neue Images automatisch, direkter Zugriff auf die Geräte ist nicht nötig.
- **Konfiguration:** Sie liegt auf dem Server in einem Docker-Volume und bleibt bei Updates erhalten. Das Board speichert die zuletzt erhaltene Konfiguration lokal.
- **Ausfallsicherheit:** Bei einem Absturz der Software, bei Verlust von Netzwerk oder Internet und bei Ausfall des Servers muss das Überleben der Pflanze jederzeit sichergestellt sein. Ausgenommen sind Stromausfälle, weil dann das gesamte System aus ist. Danach muss es sich aber selbst erholen. Die Fail-Safe-Matrix steht in [architecture.md](architecture.md).
- **Sensorausfall:** Fällt ein Sensor aus, wird er automatisch mit Back-off neu initialisiert (1 s → 2 s → 5 s → 10 s → 30 s → 60 s, danach alle 60 s). Jeder Fehler erscheint in der WebUI, ab der 10-s-Stufe kommt zusätzlich eine ntfy-Benachrichtigung.
- **Benachrichtigungen:** Sie laufen über eine selbst gehostete ntfy-Instanz auf dem Server.
- **Tests:** Zu Beginn gibt es keine automatisierten Tests.

## Physische Anforderungen
Alles soll sauber verpackt sein. Board-Computer, Netzteil und Relais sitzen in einem eigenen 3D-gedruckten Gehäuse aus **PETG** (gedruckt auf einem Ender 3 Pro). Die Sensoren hängen per Kabel daran. Das Produkt muss sich leicht zusammenstecken lassen, damit Umbauten und Umzüge leichtfallen:
- **Sensoren und Pumpe** werden mit **GX12-Rundsteckern** angeschlossen. Jeder Modultyp hat eine andere Polzahl, damit nichts verwechselt werden kann.
- In der Gehäusewand sind **USB-Ports** (für eine spätere Kamera) und eine **LAN-Buchse** nach außen geführt.
- Die Gehäuse kommen nie mit Wasser in Kontakt, eine IP-Schutzart ist nicht erforderlich.
- Die 230-V-Verdrahtung übernimmt ein ausgebildeter Elektrotechniker.

## Features
- **Lichtzyklus:** Die Lampe ist X Stunden am Tag an und 24−X Stunden aus. Die Startzeit ist konfigurierbar. Es gibt zwei Modi:
  - **Autoflower:** ein fester Zyklus für den gesamten Grow.
  - **Photoperiodisch:** frei anlegbare Profile, z. B. Wachstum 18/6 und Blüte 12/12. Gewechselt wird wahlweise manuell oder automatisch nach X Tagen.
- **Wiederherstellung des Lichtzyklus:** Nach einem Systemausfall muss der aktuelle Stand im Zyklus wiederhergestellt werden. Das Board berechnet ihn aus der Uhrzeit, die es per NTP aus dem Internet holt.
- **Temperatur:** Zieltemperatur und Hysterese sind konfigurierbar. Es gibt keine getrennten Werte für Tag und Nacht.
- **Bodenfeuchtigkeit:** Ziel-Bodenfeuchtigkeit und Hysterese sind konfigurierbar.
- **Erkennung „Eimer leer“:** Steigt die Bodenfeuchtigkeit nach mehreren Gießvorgängen nicht an, sperrt das System die Pumpe und schickt eine ntfy-Benachrichtigung. Einen Füllstandssensor gibt es nicht.

## Stromversorgung
Ein 230-V-Netzteil liefert **5 V** (Raspberry Pi, Relais-Spulen) und **12 V** (Pumpe), Varianten siehe [partslist.md](partslist.md). Die 3,3 V für Sensoren und Relais-Ansteuerung kommen vom Pi selbst. Das Gehäuse des Boards enthält **zwei geschaltete Schuko-Einbausteckdosen**, eine für die E-Heizung und eine für die Grow-Lampe.

## Ausblick (nach dem MVP)
- Kamera-Anbindung über USB
