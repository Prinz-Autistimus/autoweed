#!/usr/bin/env python3
"""Zeichnet die Übersicht der Verdrahtung im Steckplatinen-Stil (overview.svg).

Bauteile und Kabel sind hier von Hand platziert. Die verbindliche Pinbelegung
steht in controller.yml und cables.yml; bei Änderungen beide Stellen anpassen.

Aufruf: python3 overview.py [ausgabe.svg]
"""

import sys
from html import escape

W, H = 1420, 1220
FONT = "DejaVu Sans, Arial, Helvetica, sans-serif"

COLORS = {
    "RD": "#d93a2f", "BK": "#2a2a2a", "GN": "#2e9d4a", "YE": "#e8c11a",
    "BU": "#2f6fd6", "VT": "#8e44c9", "GY": "#8c9198", "WH": "#f5f5f5",
    "OG": "#f08a24",
}

out: list[str] = []


def add(s: str) -> None:
    out.append(s)


def text(x, y, s, size=12, fill="#222", anchor="start", weight="normal", rotate=None, family=FONT):
    tr = f' transform="rotate({rotate} {x} {y})"' if rotate is not None else ""
    add(f'<text x="{x}" y="{y}" font-family="{family}" font-size="{size}" fill="{fill}" '
        f'text-anchor="{anchor}" font-weight="{weight}"{tr}>{escape(s)}</text>')


def rect(x, y, w, h, fill, stroke="none", rx=0, sw=1, extra=""):
    add(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}" '
        f'stroke="{stroke}" stroke-width="{sw}" {extra}/>')


def circle(cx, cy, r, fill, stroke="none", sw=1):
    add(f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>')


def rounded_path(points, r=9):
    """Orthogonaler Linienzug mit abgerundeten Ecken."""
    d = f"M{points[0][0]:.1f},{points[0][1]:.1f}"
    for i in range(1, len(points) - 1):
        (x0, y0), (x1, y1), (x2, y2) = points[i - 1], points[i], points[i + 1]
        l1 = max(abs(x1 - x0), abs(y1 - y0))
        l2 = max(abs(x2 - x1), abs(y2 - y1))
        rr = min(r, l1 / 2, l2 / 2)

        def towards(a, b, dist):
            length = max(abs(b[0] - a[0]), abs(b[1] - a[1])) or 1
            return (b[0] + (a[0] - b[0]) * dist / length, b[1] + (a[1] - b[1]) * dist / length)

        p1 = towards((x0, y0), (x1, y1), rr)
        p2 = towards((x2, y2), (x1, y1), rr)
        d += f" L{p1[0]:.1f},{p1[1]:.1f} Q{x1:.1f},{y1:.1f} {p2[0]:.1f},{p2[1]:.1f}"
    d += f" L{points[-1][0]:.1f},{points[-1][1]:.1f}"
    return d


wires: list[tuple[str, str]] = []


def wire(color, *points, width=5):
    """Merkt ein Kabel vor; Kabel werden über den Bauteilen gezeichnet."""
    wires.append((color, rounded_path(points), width))


def pin(x, y, label=None, side="below", color="#c9a227"):
    rect(x - 4, y - 4, 8, 8, "#1b1b1b")
    circle(x, y, 2.4, color)
    if label:
        if side == "below":
            text(x, y + 16, label, 8, "#fff", "middle")
        elif side == "above":
            text(x, y - 9, label, 8, "#fff", "middle")
        elif side == "left":
            text(x - 9, y + 3, label, 9, "#fff", "end")
        elif side == "right":
            text(x + 9, y + 3, label, 9, "#fff", "start")


def screw_terminal(x, y, w, h, color="#2a8c4a"):
    rect(x, y, w, h, color, "#1d5e33", 3)
    circle(x + w / 2, y + h / 2, min(w, h) / 2 - 3, "#d9dadb", "#888", 1)
    add(f'<line x1="{x + w / 2 - 4}" y1="{y + h / 2}" x2="{x + w / 2 + 4}" y2="{y + h / 2}" stroke="#666" stroke-width="1.5"/>')


def mount_hole(x, y):
    circle(x, y, 6, "#f3f1ea", "#c9a227", 2.5)


# ---------------------------------------------------------------- Hintergrund
add(f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}">')
add("""<defs>
  <pattern id="hatch" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
    <rect width="8" height="8" fill="#fde8e6"/><line x1="0" y1="0" x2="0" y2="8" stroke="#e7a49c" stroke-width="3"/>
  </pattern>
  <linearGradient id="alu" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#e3e5e8"/><stop offset="1" stop-color="#b5b9bf"/>
  </linearGradient>
  <linearGradient id="steel" x1="0" y1="0" x2="1" y2="0">
    <stop offset="0" stop-color="#9aa0a6"/><stop offset="0.45" stop-color="#eef0f2"/><stop offset="1" stop-color="#8a9097"/>
  </linearGradient>
  <linearGradient id="water" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#9fd3f5"/><stop offset="1" stop-color="#4f9fd9"/>
  </linearGradient>
</defs>""")
rect(0, 0, W, H, "#ffffff")

# Gehäuse
rect(30, 40, 1260, 840, "#f6f5f0", "#9b998f", 18, 2, 'stroke-dasharray="10 6"')
text(48, 866, "Controller-Gehäuse (PETG)", 16, "#6b6960", weight="bold")

# 230-V-Bereiche
rect(318, 68, 58, 82, "url(#hatch)", "#d0453b", 6, 1.5)
rect(1216, 52, 176, 230, "url(#hatch)", "#d0453b", 8, 1.5)
rect(96, 6, 132, 56, "url(#hatch)", "#d0453b", 8, 1.5)
text(1300, 300, "230 V – Verdrahtung durch", 11, "#b3362d", "middle", "bold")
text(1300, 314, "die Elektrofachkraft", 11, "#b3362d", "middle", "bold")

# ---------------------------------------------------------------- Bauteile

# Kaltgerätebuchse C14 (Gehäusewand oben)
rect(118, 16, 88, 36, "#232323", rx=5)
rect(132, 22, 36, 24, "#3a3a3a", rx=4)
for dx in (140, 150, 160):
    rect(dx - 2, 28, 4, 10, "#c9a227")
rect(176, 24, 22, 20, "#b8322b", rx=3)
text(162, 76, "C14 + Sicherung + Schalter", 10, "#444", "middle")

# Netzteil 12 V
rect(60, 80, 260, 160, "url(#alu)", "#8a8f96", 6, 1.5)
for i in range(9):
    rect(78 + i * 24, 98, 12, 70, "#9ca1a8", rx=3)
text(190, 200, "Netzteil 230 V → 12 V", 14, "#333", "middle", "bold")
text(190, 218, "6 A / 75 W", 12, "#555", "middle")
PSU_T = {}
for i, name in enumerate(["L", "N", "PE", "-V", "-V", "+V", "+V"]):
    y = 84 + i * 22
    rect(320, y, 30, 20, "#2d2d2d", rx=2)
    circle(335, y + 10, 6, "#d9dadb", "#777")
    text(314, y + 14, name, 10, "#333" if i > 2 else "#b3362d", "end", "bold")
    PSU_T.setdefault(name, []).append((350, y + 10))

# DC-DC-Wandler (vergossenes Modul mit Litzen)
rect(440, 92, 160, 96, "#1f1f1f", "#000", 8)
text(520, 130, "DC-DC", 15, "#fff", "middle", "bold")
text(520, 149, "12 V → 5 V  5 A", 11, "#ddd", "middle")
text(520, 166, "vor Anschluss messen!", 9, "#f0b429", "middle")
DCDC = {"IN+": (440, 112), "IN-": (440, 168), "OUT+": (600, 112), "OUT-": (600, 168)}
for name, (x, y) in DCDC.items():
    text(x + (8 if x == 440 else -8), y + 4, name, 9, "#bbb", "start" if x == 440 else "end")

# Lochraster-Verteiler
rect(70, 350, 300, 118, "#d9bf86", "#a8894d", 6, 1.5)
text(78, 484, "Lochraster-Verteiler", 11, "#6b5426", "start", "bold")
RAILS = {"3V3": 372, "GND": 397, "SDA": 422, "SCL": 447}
RAIL_X = [100 + i * 22 for i in range(12)]
for name, y in RAILS.items():
    rect(88, y - 6, 270, 12, "#c58b43", rx=3)
    for x in RAIL_X:
        circle(x, y, 3, "#f1e2bd", "#8d6a2e", 1)
    text(82, y + 4, name, 10, "#4b3a17", "end", "bold")

# ADS1115
rect(86, 250, 226, 82, "#3c3a9c", "#26246e", 6, 1.5)
text(199, 280, "ADS1115", 15, "#fff", "middle", "bold")
text(199, 296, "16-Bit ADC · I²C 0x48", 9, "#d6d4ff", "middle")
ADS_NAMES = ["VDD", "GND", "SCL", "SDA", "ADDR", "ALRT", "A0", "A1", "A2", "A3"]
ADS = {}
for i, name in enumerate(ADS_NAMES):
    x = RAIL_X[i]
    pin(x, 322, name, "above")
    ADS[name] = (x, 322)

# Raspberry Pi 3
PI_X, PI_Y = 430, 330
rect(PI_X, PI_Y, 425, 280, "#1d8a3c", "#12652a", 14, 2)
for mx, my in ((PI_X + 17, PI_Y + 17), (PI_X + 307, PI_Y + 17), (PI_X + 17, PI_Y + 263), (PI_X + 307, PI_Y + 263)):
    mount_hole(mx, my)
rect(PI_X + 150, PI_Y + 110, 70, 70, "#222", rx=4)
rect(PI_X + 160, PI_Y + 120, 50, 50, "#333", rx=3)
text(PI_X + 185, PI_Y + 210, "Raspberry Pi 3", 16, "#e8f5ea", "middle", "bold")
rect(PI_X + 400, PI_Y + 18, 32, 58, "#c7cbd0", "#8a8f96", 3)
rect(PI_X + 400, PI_Y + 92, 32, 58, "#c7cbd0", "#8a8f96", 3)
rect(PI_X + 392, PI_Y + 170, 40, 74, "#c7cbd0", "#8a8f96", 3)
text(PI_X + 386, PI_Y + 52, "USB", 9, "#e8f5ea", "end")
text(PI_X + 386, PI_Y + 126, "USB", 9, "#e8f5ea", "end")
text(PI_X + 386, PI_Y + 212, "LAN", 9, "#e8f5ea", "end")
rect(PI_X + 40, PI_Y + 268, 26, 14, "#c7cbd0", "#8a8f96", 2)
text(PI_X + 53, PI_Y + 262, "5 V", 9, "#e8f5ea", "middle")
rect(PI_X + 140, PI_Y + 262, 60, 20, "#c7cbd0", "#8a8f96", 2)
text(PI_X + 170, PI_Y + 256, "HDMI", 9, "#e8f5ea", "middle")
# GPIO-Leiste: ungerade Pins innen (unten), gerade außen (oben), Pin 1 links
HEADER_X0, PITCH = PI_X + 38, 12.7
rect(HEADER_X0 - 8, PI_Y + 6, 20 * PITCH + 4, 32, "#141414", rx=2)
PI = {}
for n in range(1, 41):
    col = (n - 1) // 2
    x = HEADER_X0 + col * PITCH
    y = PI_Y + 28 if n % 2 else PI_Y + 15
    circle(x, y, 3, "#c9a227")
    PI[n] = (x, y)
rect(PI[1][0] - 4, PI[1][1] - 4, 8, 8, "none", "#fff", sw=1)
text(PI[1][0], PI_Y + 50, "1", 9, "#fff", "middle")
text(HEADER_X0 - 6, PI_Y + 66, "Pin 1: 3V3 · 3: SDA · 5: SCL · 6: GND · 17: 3V3", 10, "#e8f5ea")
text(HEADER_X0 - 6, PI_Y + 80, "29: GPIO5 · 31: GPIO6 · 33: GPIO13 · 34: GND", 10, "#e8f5ea")
PI_USB = (PI_X + 53, PI_Y + 282)

# Relais-Modul
RX, RY = 930, 56
rect(RX, RY, 300, 180, "#2667b0", "#174a84", 8, 1.5)
for i, ry in enumerate((RY + 18, RY + 98)):
    rect(RX + 70, ry, 120, 66, "#1d56c9", "#123c8f", 4, 1.5)
    text(RX + 130, ry + 30, "SRD-05VDC", 10, "#e6eeff", "middle", "bold")
    text(RX + 130, ry + 46, f"K{i + 1} – {'Heizung' if i == 0 else 'Lampe'}", 10, "#e6eeff", "middle")
    for j, name in enumerate(("NO", "COM", "NC")):
        screw_terminal(RX + 262, ry + j * 22, 32, 22)
        text(RX + 256, ry + j * 22 + 15, name, 8, "#fff", "end")
text(RX + 130, RY + 176, "Relais-Modul 2-Kanal (schaltet bei LOW)", 10, "#e6eeff", "middle")
REL = {}
for i, name in enumerate(("GND", "IN1", "IN2", "VCC")):
    y = RY + 24 + i * 16
    pin(RX + 14, y, name, "right")
    REL[name] = (RX + 14, y)
for i, name in enumerate(("GND", "VCC", "JD-VCC")):
    y = RY + 112 + i * 16
    pin(RX + 14, y, name, "right")
    REL["JD:" + name] = (RX + 14, y)
text(RX + 60, RY + 166, "Jumper entfernt", 8, "#ffd27a", "start")

# MOSFET-Modul
MX, MY = 1000, 640
rect(MX, MY, 190, 96, "#1e4f8f", "#123562", 6, 1.5)
rect(MX + 80, MY + 30, 30, 30, "#222", rx=2)
text(MX + 95, MY + 76, "D4184", 11, "#fff", "middle", "bold")
MOS = {}
for i, name in enumerate(("VIN+", "VIN-")):
    screw_terminal(MX + 10 + i * 30, MY - 22, 30, 24, "#1f6fbf")
    MOS[name] = (MX + 25 + i * 30, MY - 22)
    text(MX + 25 + i * 30, MY + 14, name, 8, "#fff", "middle")
for i, name in enumerate(("OUT+", "OUT-")):
    screw_terminal(MX + 120 + i * 30, MY + 94, 30, 24, "#1f6fbf")
    MOS[name] = (MX + 135 + i * 30, MY + 118)
    text(MX + 135 + i * 30, MY + 88, name, 8, "#fff", "middle")
for i, name in enumerate(("TRIG", "GND")):
    pin(MX + 12, MY + 40 + i * 18, name, "right")
    MOS[name] = (MX + 12, MY + 40 + i * 18)

# Schuko-Dosen (rechte Wand)
for i, (label, y) in enumerate((("Heizung (K1)", 64), ("Lampe (K2)", 172))):
    rect(1300, y, 72, 72, "#fbfbfb", "#9a9a9a", 10, 1.5)
    circle(1336, y + 36, 26, "#ececec", "#b5b5b5", 1.5)
    circle(1326, y + 36, 4, "#333")
    circle(1346, y + 36, 4, "#333")
    rect(1332, y + 10, 8, 5, "#c9a227")
    rect(1332, y + 57, 8, 5, "#c9a227")
    text(1336, y + 88, label, 10, "#444", "middle", "bold")

# Einbaubuchsen LAN und USB (rechte Wand)
for label, y in (("LAN", 520), ("USB (Kamera)", 590)):
    rect(1280, y - 16, 22, 32, "#555", rx=3)
    text(1310, y + 4, label, 10, "#444", "start")

# GX12-Buchsen (untere Wand)
GX = {"CLIMATE": (520, 880, 4, "GX12-4 · SHT40"), "SOIL": (720, 880, 3, "GX12-3 · Boden"),
      "PUMP": (1155, 880, 2, "GX12-2 · Pumpe")}
GX_PINS = {}
for key, (cx, cy, n, label) in GX.items():
    circle(cx, cy, 25, "#c7cbd0", "#7d828a", 2)
    circle(cx, cy, 16, "#3a3a3a")
    offsets = [(-(n - 1) / 2 + i) * 9 for i in range(n)]
    for i, dx in enumerate(offsets):
        circle(cx + dx, cy - 3, 2.5, "#c9a227")
        GX_PINS[f"{key}:{i + 1}"] = (cx + dx, cy - 25)
    text(cx + 32, cy + 22, label, 10, "#444", "start", "bold")

# ---------------------------------------------------------------- Außen
text(48, 920, "Außerhalb des Gehäuses", 14, "#6b6960", weight="bold")


def plug(cx, top):
    rect(cx - 15, top, 30, 32, "#b9bec4", "#7d828a", 4, 1.5)
    rect(cx - 9, top + 32, 18, 14, "#2a2a2a", rx=3)


def cable(*points, width=9):
    add(f'<path d="{rounded_path(points, 30)}" fill="none" stroke="#1f1f1f" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round"/>')


# SHT40-Sonde
plug(520, 906)
cable((520, 952), (520, 1090))
rect(512, 1090, 16, 30, "#2a2a2a", rx=3)
rect(510, 1120, 20, 64, "url(#steel)", "#7d828a", 5, 1)
text(546, 1140, "SHT40-Sonde", 12, "#333", "start", "bold")
text(546, 1156, "hängt oben im Zelt", 10, "#666", "start")

# Topf mit Bodensensor
add('<path d="M640,1080 L840,1080 L820,1190 L660,1190 Z" fill="#b5642c" stroke="#8a4a1f" stroke-width="2"/>')
rect(646, 1074, 188, 14, "#4a3424", rx=4)
add('<path d="M748,1074 C730,1030 700,1020 690,1000 C720,1004 744,1030 752,1060 C760,1030 790,1000 820,996 C808,1024 772,1036 756,1074 Z" fill="#3f9d3a"/>')
plug(720, 906)
cable((720, 952), (720, 990), (680, 990), (680, 1040))
add('<path d="M666,1040 L694,1040 L694,1110 L686,1130 L686,1110 L674,1110 L674,1130 L666,1110 Z" fill="#1c1c1c"/>')
text(852, 1150, "Bodenfeuchte", 12, "#333", "start", "bold")
text(852, 1166, "kapazitiv v1.2", 10, "#666", "start")

# Eimer mit Pumpe
add('<path d="M1080,1040 L1240,1040 L1228,1192 L1092,1192 Z" fill="#f2f2f2" stroke="#9a9a9a" stroke-width="2"/>')
add('<path d="M1084,1070 L1236,1070 L1228,1188 L1092,1188 Z" fill="url(#water)" opacity="0.85"/>')
rect(1130, 1130, 52, 44, "#555b62", "#33373c", 8, 1.5)
text(1156, 1158, "12 V", 10, "#fff", "middle", "bold")
plug(1155, 906)
cable((1155, 952), (1155, 1130))
add('<path d="M1172,1130 C1172,1010 980,1000 820,1060" fill="none" stroke="#7fb7e0" stroke-width="7" opacity="0.8" stroke-linecap="round"/>')
text(1260, 1120, "Eimer mit", 12, "#333", "start", "bold")
text(1260, 1136, "Dünger-Lösung", 12, "#333", "start", "bold")
text(1260, 1152, "Tauchpumpe 12 V", 10, "#666", "start")

# ---------------------------------------------------------------- Kabel (innen)
R = COLORS
# 12 V
wire(R["RD"], PSU_T["+V"][0], (400, PSU_T["+V"][0][1]), (400, 112), DCDC["IN+"])
wire(R["BK"], PSU_T["-V"][0], (390, PSU_T["-V"][0][1]), (390, 168), DCDC["IN-"])
wire(R["RD"], PSU_T["+V"][1], (380, PSU_T["+V"][1][1]), (380, 252), (MOS["VIN+"][0], 252), MOS["VIN+"])
wire(R["BK"], PSU_T["-V"][1], (372, PSU_T["-V"][1][1]), (372, 244), (MOS["VIN-"][0], 244), MOS["VIN-"])
# 5 V
wire(R["RD"], DCDC["OUT+"], (884, 112), (884, 640), (PI_USB[0], 640), PI_USB)
wire(R["BK"], DCDC["OUT-"], (892, 168), (892, 648), (PI_USB[0] + 8, 648), (PI_USB[0] + 8, PI_USB[1]))
wire(R["OG"], DCDC["OUT+"], (620, 112), (620, 64), (912, 64), (912, REL["JD:JD-VCC"][1]), REL["JD:JD-VCC"])
wire(R["BK"], DCDC["OUT-"], (628, 168), (628, 72), (904, 72), (904, REL["JD:GND"][1]), REL["JD:GND"])
# Pi → Verteiler
rail_end = RAIL_X[-1]
wire(R["RD"], PI[1], (PI[1][0], 300), (396, 300), (396, RAILS["3V3"]), (rail_end, RAILS["3V3"]))
wire(R["BK"], PI[6], (PI[6][0], 292), (404, 292), (404, RAILS["GND"]), (rail_end, RAILS["GND"]))
wire(R["GN"], PI[3], (PI[3][0], 284), (412, 284), (412, RAILS["SDA"]), (rail_end, RAILS["SDA"]))
wire(R["YE"], PI[5], (PI[5][0], 276), (420, 276), (420, RAILS["SCL"]), (rail_end, RAILS["SCL"]))
# Verteiler → ADS1115
for name, rail, color in (("VDD", "3V3", "RD"), ("GND", "GND", "BK"), ("SCL", "SCL", "YE"), ("SDA", "SDA", "GN")):
    x = ADS[name][0]
    wire(R[color], (x, RAILS[rail]), ADS[name])
wire(R["BK"], ADS["ADDR"], (ADS["ADDR"][0], 340), (ADS["GND"][0] + 6, 340), (ADS["GND"][0] + 6, RAILS["GND"]), width=3)
# 1 MΩ von A0 nach GND: abgezogener Bodensensor liest sicher 0
RES_X = ADS["A0"][0] + 11
wire(R["BK"], (ADS["A0"][0], 338), (RES_X, 338), (RES_X, RAILS["GND"]), width=2)
# Pi 3V3 (Pin 17) → Relais VCC (Optokoppler-Seite)
wire(R["RD"], PI[17], (PI[17][0], 284), (920, 284), (920, REL["VCC"][1]), REL["VCC"])
# Pi → Relais IN1/IN2
wire(R["BU"], PI[29], (PI[29][0], 300), (908, 300), (908, REL["IN1"][1]), REL["IN1"])
wire(R["VT"], PI[31], (PI[31][0], 308), (900, 308), (900, REL["IN2"][1]), REL["IN2"])
# Pi → MOSFET
wire(R["GY"], PI[33], (PI[33][0], 316), (960, 316), (960, MOS["TRIG"][1]), MOS["TRIG"])
# Pin 34 liegt in derselben Spalte wie Pin 33, daher ein kleiner Versatz.
wire(R["BK"], PI[34], (PI[34][0] + 7, PI[34][1] - 7), (PI[34][0] + 7, 268), (968, 268), (968, MOS["GND"][1]), MOS["GND"])
# MOSFET → Pumpenbuchse
wire(R["RD"], MOS["OUT+"], (MOS["OUT+"][0], 820), (GX_PINS["PUMP:1"][0], 820), GX_PINS["PUMP:1"])
wire(R["BK"], MOS["OUT-"], (MOS["OUT-"][0], 830), (GX_PINS["PUMP:2"][0], 830), GX_PINS["PUMP:2"])
# Verteiler → SHT40-Buchse
for i, (rail, color, col) in enumerate((("3V3", "RD", 6), ("GND", "BK", 7), ("SDA", "GN", 8), ("SCL", "YE", 9))):
    x = RAIL_X[col]
    target = GX_PINS[f"CLIMATE:{i + 1}"]
    wire(R[color], (x, RAILS[rail]), (x, 500 + i * 9), (target[0], 500 + i * 9), target)
# Verteiler/ADS → Bodenbuchse
for i, (start, color) in enumerate((((RAIL_X[4], RAILS["3V3"]), "RD"), ((RAIL_X[5], RAILS["GND"]), "BK"), (ADS["A0"], "WH"))):
    target = GX_PINS[f"SOIL:{i + 1}"]
    y = 548 + i * 9
    wire(R[color], start, (start[0], y), (target[0], y), target)
# Patchkabel LAN / USB
wire("#9aa3ad", (PI_X + 432, PI_Y + 207), (1280, PI_Y + 207), (1280, 520), width=7)
wire("#9aa3ad", (PI_X + 432, PI_Y + 47), (870, PI_Y + 47), (870, 590), (1280, 590), width=7)

for color, d, width in wires:
    add(f'<path d="{d}" fill="none" stroke="#000" stroke-opacity="0.35" stroke-width="{width + 2.5}" stroke-linecap="round" stroke-linejoin="round"/>')
    add(f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round"/>')

# Widerstandskörper über dem Kabel
rect(RES_X - 4, 343, 8, 22, "#d8c08f", "#8a6d3b", 2, 1)
for i, c in enumerate(("#8b4513", "#2a2a2a", "#2e9d4a")):  # braun schwarz grün = 1 MΩ
    rect(RES_X - 4, 347 + i * 5, 8, 2, c)
text(RES_X + 7, 358, "1 MΩ", 8, "#4b3a17", "start", "bold")

# ---------------------------------------------------------------- Legende
LX, LY = 30, 960
rect(LX, LY, 360, 236, "#fafaf7", "#cfcdc4", 10, 1.5)
text(LX + 16, LY + 26, "Aderfarben", 13, "#333", weight="bold")
legend = [("RD", "Plus: 3,3 V / 5 V / 12 V"), ("BK", "GND / Minus"), ("GN", "I²C SDA"), ("YE", "I²C SCL"),
          ("OG", "5 V Relais-Spulen (JD-VCC)"), ("BU", "GPIO5 → Relais Heizung"),
          ("VT", "GPIO6 → Relais Lampe"), ("GY", "GPIO13 → MOSFET Pumpe"), ("WH", "Bodenfeuchte-Signal → A0")]
for i, (code, label) in enumerate(legend):
    y = LY + 48 + i * 20
    add(f'<line x1="{LX + 18}" y1="{y}" x2="{LX + 52}" y2="{y}" stroke="#000" stroke-opacity="0.35" stroke-width="7.5" stroke-linecap="round"/>')
    add(f'<line x1="{LX + 18}" y1="{y}" x2="{LX + 52}" y2="{y}" stroke="{COLORS[code]}" stroke-width="5" stroke-linecap="round"/>')
    text(LX + 64, y + 4, label, 11, "#333")

text(W - 20, H - 12, "AutoWeed · Übersicht der Verdrahtung · Details: controller.yml / cables.yml", 10, "#999", "end")
add("</svg>")

path = sys.argv[1] if len(sys.argv) > 1 else "overview.svg"
with open(path, "w", encoding="utf-8") as f:
    f.write("\n".join(out))
print(f"geschrieben: {path}")
