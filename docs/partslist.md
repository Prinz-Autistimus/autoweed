# Teileliste

## Vorhanden

| Teil | Bemerkung |
|---|---|
| Grow-Zelt | bis 1,8 m |
| Grow-Lampe | Leistung: **? W** |
| E-Heizung | 500–1000 W, Überhitzungsschutz eingebaut (mit Elektrotechniker abgestimmt) |
| Luftfilter mit Lüfter | läuft unabhängig, wird nicht gesteuert |
| Töpfe | |
| Wasserpumpe | 12 V Tauchpumpe, generisch |
| Eimer | für Wasser-Dünger-Lösung |
| Raspberry Pi 3 | mit WLAN + Bluetooth; Board-Computer |
| Devolo-Adapter | LAN in die Garage |
| 3D-Drucker | Ender 3 Pro mit OctoPrint |

## Zu kaufen

| Teil | Menge | Bemerkung |
|---|---|---|
| microSD 32 GB, „High Endurance“ | 1 | hält Dauerschreiben besser aus |
| Micro-USB-Kabel mit offenen Enden (Pigtail) | 1 | Pi vom 5-V-Ausgang des Netzteils versorgen; nicht über die GPIO-Pins, sonst fehlt die Schutzschaltung des Pi |
| Netzteil 230 V → **5 V + 12 V** | 1 | **Variante offen**, siehe Hinweise. 5 V ≥ 3 A (Pi 3 braucht 2,5 A, Reserve für die Kamera); 12 V passend zum Pumpenstrom |
| Relais-Modul 2-Kanal, 5-V-Spulen, 10 A / 250 VAC, mit Optokoppler und **JD-VCC-Jumper** | 1 | Heizung + Lampe; VCC an 3,3 V vom Pi, JD-VCC an 5 V, siehe Hinweise |
| MOSFET-Modul, Logic-Level (z. B. D4184 / AOD4184) | 1 | Pumpe 12 V; **kein IRF520** (schaltet bei 3,3 V nicht richtig durch) |
| Temperatur-/Feuchtesensor SHT31 oder SHT40 (I²C-Breakout) | 1 | hängt oben im Zelt |
| Kapazitiver Bodenfeuchtesensor (v1.2, analog) | 1 | 3,3 V versorgen, damit das Signal im ADC-Bereich bleibt |
| ADS1115 (ADC, I²C) | 1 | im Controllergehäuse; der Pi hat keine Analogeingänge |
| Schuko-Einbausteckdose | 2 | Heizung, Lampe |
| Kaltgeräte-Einbaubuchse C14 mit Sicherung + Schalter | 1 | 230-V-Eingang |
| GX12 Stecker/Buchse 2-polig | 1 Paar | Pumpe |
| GX12 Stecker/Buchse 3-polig | 1 Paar | Bodensensor |
| GX12 Stecker/Buchse 4-polig | 1 Paar | Temperatur/Feuchte |
| USB-A-Einbau-Verlängerung | 1–2 | für die spätere Kamera |
| RJ45-Einbau-Verlängerung | 1 | LAN-Buchse in der Gehäusewand |
| PETG-Filament | 1 kg | |
| Kleinmaterial | | Litze, Aderendhülsen, Schrumpfschlauch, Schrauben/Einschmelzmuttern |

## Hinweise
- Heizung (500–1000 W) und Relais (10 A) sind mit dem Elektrotechniker abgestimmt. LED-Lampen haben einen hohen Einschaltstrom; das bei der Lampe berücksichtigen.
- **3,3 V liefert der Pi selbst** (Pin 1/17). Das reicht für Temperatursensor, ADS1115, Bodensensor und die Optokoppler-Seite des Relais-Moduls (zusammen nur wenige mA). Das Netzteil braucht also nur 5 V und 12 V.
- **Netzteil-Varianten:**
  - Ein Doppel-Netzteil 5 V + 12 V (z. B. Mean Well RD-Serie). Zertifiziert, ein Gerät, aber teurer.
  - Ein 12-V-Netzteil plus DC-DC-Abwärtswandler 12 → 5 V (≥ 3 A Dauerstrom). Am günstigsten. Ausgang vor dem Anschließen des Pi auf 5,1 V einstellen und nachmessen.
  - Zwei getrennte Netzteile (5 V und 12 V).
- **Warum 3,3-V-Logik:** Die GPIOs des Pi arbeiten mit 3,3 V und sind **nicht 5-V-tolerant**. Mit dem JD-VCC-Aufbau sieht der GPIO nie 5 V, und die Relais fallen bei „aus“ sauber ab.
