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

Stand der Bestellung: ✅ bestellt (AliExpress und Amazon, 31.10.2026), ❌ fehlt noch.

| | Teil | Menge | Bemerkung |
|---|---|---|---|
| ❌ | microSD 32 GB, „High Endurance“ | 1 | hält Dauerschreiben besser aus |
| ❌ | Micro-USB-Kabel mit offenen Enden (Pigtail) | 1 | Pi vom 5-V-Ausgang des Netzteils versorgen; nicht über die GPIO-Pins, sonst fehlt die Schutzschaltung des Pi |
| ✅ | Netzteil 230 V → **5 V + 12 V** | 1 | **Bestellt:** 12 V 6 A (75 W) + DC-DC 12 → 5 V 5 A. 5 V ≥ 3 A (Pi 3 braucht 2,5 A, Reserve für die Kamera); 12 V passend zum Pumpenstrom |
| ❌ | Relais-Modul 2-Kanal, 5-V-Spulen, 10 A / 250 VAC, mit Optokoppler und **JD-VCC-Jumper** | 1 | Heizung + Lampe; VCC an 3,3 V vom Pi, JD-VCC an 5 V, siehe Hinweise |
| ✅ | MOSFET-Modul, Logic-Level (z. B. D4184 / AOD4184) | 1 | **Bestellt:** D4184. Pumpe 12 V; **kein IRF520** (schaltet bei 3,3 V nicht richtig durch) |
| ✅ | SHT40-Sonde (Edelstahl, I²C, 4 Adern, ca. 1 m Kabel) | 1 | **Bestellt:** (Kabellänge bei Lieferung prüfen). hängt oben im Zelt; an 3,3 V vom Pi, **nicht an 5 V**; Adresse 0x44 |
| ✅ | Kapazitiver Bodenfeuchtesensor (v1.2, analog) | 1 | **Bestellt:** (Chip bei Lieferung prüfen: TLC555?). 3,3 V versorgen; auf **TLC555** achten (NE555-Varianten laufen an 3,3 V oft nicht). Keine resistiven Sensoren (Korrosion, Dünger verfälscht den Wert) |
| ✅ | ADS1115 (ADC, I²C) | 1 | im Controllergehäuse; der Pi hat keine Analogeingänge |
| ❌ | Schuko-Einbausteckdose | 2 | Heizung, Lampe |
| ✅ | Kaltgeräte-Einbaubuchse C14 mit Sicherung + Schalter | 1 | **Bestellt:** Amazon. 230-V-Eingang |
| ❌ | GX12 Stecker/Buchse 2-polig | 1 Paar | Pumpe |
| ❌ | GX12 Stecker/Buchse 3-polig | 1 Paar | Bodensensor |
| ❌ | GX12 Stecker/Buchse 4-polig | 1 Paar | Temperatur/Feuchte |
| ❌ | USB-A-Einbau-Verlängerung | 1–2 | für die spätere Kamera |
| ✅ | RJ45-Einbau-Verlängerung | 1 | **Bestellt:** 20 cm. LAN-Buchse in der Gehäusewand |
| ✅ | PETG-Filament | 1 kg | **Bestellt:** SUNLU matt schwarz. |
| ✅ | Einschmelzmuttern M3 (Länge ca. 4–5,7 mm) | ~50 | **Bestellt:** Set M2/M3/M4 mit Schrauben. Gehäuse, Deckel, Relais-Modul, Netzteil, MOSFET-Modul |
| ✅ | Einschmelzmuttern M2.5 (Länge ca. 4 mm) | ~10 | **Bestellt:** 500 Stück. Raspberry Pi (Bohrungen 2,7 mm), ADS1115 |
| ✅ | Zylinderschrauben M3, DIN 912, Edelstahl A2, 6/8/10/12/16 mm | Sortiment | **Bestellt:** im Einschmelzmuttern-Set (Kreuzschlitz). |
| ❌ | Zylinderschrauben M2.5, DIN 912, Edelstahl A2, 6/8 mm | Sortiment | |
| ❌ | Kleinmaterial | | Litze, Aderendhülsen, Schrumpfschlauch, Kabelbinder |

## Hinweise
- Heizung (500–1000 W) und Relais (10 A) sind mit dem Elektrotechniker abgestimmt. LED-Lampen haben einen hohen Einschaltstrom; das bei der Lampe berücksichtigen.
- **3,3 V liefert der Pi selbst** (Pin 1/17). Das reicht für Temperatursensor, ADS1115, Bodensensor und die Optokoppler-Seite des Relais-Moduls (zusammen nur wenige mA). Das Netzteil braucht also nur 5 V und 12 V.
- **Netzteil-Varianten:**
  - Ein Doppel-Netzteil 5 V + 12 V (z. B. Mean Well RD-Serie). Zertifiziert, ein Gerät, aber teurer.
  - Ein 12-V-Netzteil plus DC-DC-Abwärtswandler 12 → 5 V (≥ 3 A Dauerstrom). Am günstigsten. Ausgang vor dem Anschließen des Pi auf 5,1 V einstellen und nachmessen.
  - Zwei getrennte Netzteile (5 V und 12 V).
- **Warum 3,3-V-Logik:** Die GPIOs des Pi arbeiten mit 3,3 V und sind **nicht 5-V-tolerant**. Mit dem JD-VCC-Aufbau sieht der GPIO nie 5 V, und die Relais fallen bei „aus“ sauber ab.
