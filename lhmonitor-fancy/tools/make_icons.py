# Iconos del plugin: lista de acciones (blanco sobre transparente) y teclas por defecto.
import math
from PIL import Image, ImageDraw, ImageFont

OUT = "com.malzar.lhmmonitor.sdPlugin/imgs/"
BOLD = "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed-Bold.ttf"
SS = 8  # supersampling

def glyph(d, k, s, col, lw):
    # s: escala (px por unidad de una rejilla 24×24)
    P = lambda *a: [v * s for v in a]
    W = max(1, int(lw * s))
    if k == "cpu":
        d.rounded_rectangle(P(6, 6, 18, 18), radius=1.5 * s, outline=col, width=W)
        d.rectangle(P(9.5, 9.5, 14.5, 14.5), fill=col)
        for i in (9, 12, 15):
            d.line(P(i, 2.5, i, 6), fill=col, width=W); d.line(P(i, 18, i, 21.5), fill=col, width=W)
            d.line(P(2.5, i, 6, i), fill=col, width=W); d.line(P(18, i, 21.5, i), fill=col, width=W)
    elif k == "gpu":
        d.rounded_rectangle(P(3, 6, 21, 17), radius=1.5 * s, outline=col, width=W)
        d.ellipse(P(9, 7.5, 17, 15.5), outline=col, width=W)
        d.ellipse(P(12, 10.5, 14, 12.5), fill=col)
        d.line(P(5, 17, 5, 20), fill=col, width=W); d.line(P(9, 17, 16, 17), fill=col, width=W)
        d.line(P(9, 17, 9, 19.5), fill=col, width=W); d.line(P(16, 17, 16, 19.5), fill=col, width=W)
    elif k == "mem":
        d.rounded_rectangle(P(2.5, 7, 21.5, 15), radius=1 * s, outline=col, width=W)
        for x in (5.5, 9.5, 13.5, 17.5):
            d.rectangle(P(x - 0.2, 9, x + 1.8, 13), fill=col)
        for x in range(4, 21, 2):
            d.line(P(x, 15, x, 18), fill=col, width=max(1, W // 2 + 1))
    elif k == "disk":
        for y in (5, 10.5, 16):
            d.rounded_rectangle(P(4, y, 20, y + 4.5), radius=1.5 * s, outline=col, width=W)
            d.ellipse(P(16, y + 1.6, 17.4, y + 3), fill=col)
    elif k == "net":
        d.line(P(8, 19, 8, 5), fill=col, width=W); d.polygon(P(4, 9, 8, 4, 12, 9), fill=col)
        d.line(P(16, 5, 16, 19), fill=col, width=W); d.polygon(P(12, 15, 16, 20, 20, 15), fill=col)
    elif k == "fan":
        d.ellipse(P(10, 10, 14, 14), fill=col)
        for a in (0, 120, 240):
            r = math.radians(a)
            pts = []
            for t in range(0, 101, 5):
                tt = t / 100
                ang = r + tt * 1.6
                rad = 2 + tt * 8.5
                pts.append((12 + rad * math.cos(ang), 12 + rad * math.sin(ang)))
            for t in range(100, -1, -5):
                tt = t / 100
                ang = r + tt * 1.6 + 0.55 * (1 - abs(2 * tt - 1))
                rad = 2 + tt * 8.5
                pts.append((12 + rad * math.cos(ang), 12 + rad * math.sin(ang)))
            d.polygon([(x * s, y * s) for x, y in pts], fill=col)
    elif k == "board":
        d.rounded_rectangle(P(3, 3, 21, 21), radius=2 * s, outline=col, width=W)
        d.rectangle(P(7, 7, 12, 12), outline=col, width=W)
        d.line(P(12, 9.5, 17, 9.5), fill=col, width=W); d.ellipse(P(16, 8, 19, 11), fill=col)
        d.line(P(9.5, 12, 9.5, 17), fill=col, width=W); d.line(P(9.5, 17, 17, 17), fill=col, width=W)
        d.ellipse(P(16, 15.5, 19, 18.5), fill=col)
    elif k == "lhm":
        pts = [(2, 13), (6, 13), (8.5, 7), (12, 18), (15, 10), (17, 13), (22, 13)]
        d.line([(x * s, y * s) for x, y in pts], fill=col, width=W, joint="curve")

def mono(k, size):
    S = size * SS
    im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    glyph(ImageDraw.Draw(im), k, S / 24, (255, 255, 255, 255), 1.8)
    return im.resize((size, size), Image.LANCZOS)

ACCENT = {"cpu": (93, 202, 165), "gpu": (133, 183, 235), "mem": (175, 169, 236), "disk": (239, 159, 39),
          "net": (93, 202, 165), "fan": (133, 183, 235), "board": (240, 153, 123), "lhm": (93, 202, 165)}
NAMES = {"cpu": "CPU", "gpu": "GPU", "mem": "RAM", "disk": "Disco", "net": "Red", "fan": "Fans", "board": "Placa"}

def key(k, size):
    S = size * SS
    im = Image.new("RGB", (S, S), (14, 17, 22))
    d = ImageDraw.Draw(im)
    u = S / 144
    sub = Image.new("RGBA", (int(64 * u), int(64 * u)), (0, 0, 0, 0))
    glyph(ImageDraw.Draw(sub), k, 64 * u / 24, ACCENT[k] + (255,), 1.7)
    im.paste(sub, (int(40 * u), int(22 * u)), sub)
    if k in NAMES:
        f = ImageFont.truetype(BOLD, int(22 * u))
        d.text((S / 2, 116 * u), NAMES[k], font=f, fill=(232, 237, 242), anchor="mm")
    return im.resize((size, size), Image.LANCZOS)

for k in ["cpu", "gpu", "mem", "disk", "net", "fan", "board"]:
    mono(k, 20).save(OUT + f"act_{k}.png"); mono(k, 40).save(OUT + f"act_{k}@2x.png")
    key(k, 72).save(OUT + f"key_{k}.png"); key(k, 144).save(OUT + f"key_{k}@2x.png")
mono("lhm", 28).save(OUT + "category.png"); mono("lhm", 56).save(OUT + "category@2x.png")
key("lhm", 72).save(OUT + "plugin.png"); key("lhm", 144).save(OUT + "plugin@2x.png")

# Hoja de muestra para revisar
sheet = Image.new("RGB", (8 * 150, 150 + 60), (60, 60, 60))
for i, k in enumerate(["cpu", "gpu", "mem", "disk", "net", "fan", "board", "lhm"]):
    sheet.paste(key(k, 144), (i * 150 + 3, 3))
    m = mono(k, 40); bg = Image.new("RGB", (40, 40), (40, 40, 40)); bg.paste(m, (0, 0), m)
    sheet.paste(bg, (i * 150 + 55, 160))
sheet.save("test/icons_sheet.png")
print("ok")
