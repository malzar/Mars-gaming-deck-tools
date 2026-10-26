# Genera lib/font.js: atlas de glifos (máscaras alfa) a partir de DejaVu Sans Condensed.
# DejaVu: licencia libre (Bitstream Vera / dominio público), se puede redistribuir.
import base64, json, zlib
from PIL import Image, ImageDraw, ImageFont

REG = "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed.ttf"
BOLD = "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed-Bold.ttf"
FULL = "".join(chr(c) for c in range(32, 127)) + "áéíóúÁÉÍÓÚñÑüÜçÇàèìòùÀÈÌÒÙâêîôûºª°·—…¿¡µ€"
DIGITS = "0123456789.,-—+ %"
VARIANTS = {
    "r16": (REG, 16, FULL), "r18": (REG, 18, FULL), "r20": (REG, 20, FULL),
    "b20": (BOLD, 20, FULL), "b22": (BOLD, 22, FULL), "b24": (BOLD, 24, FULL),
    "b32": (BOLD, 32, DIGITS), "b40": (BOLD, 40, DIGITS), "b48": (BOLD, 48, DIGITS), "b56": (BOLD, 56, DIGITS),
}

out = {}
for name, (path, size, chars) in VARIANTS.items():
    f = ImageFont.truetype(path, size)
    asc, desc = f.getmetrics()
    glyphs, blob = {}, bytearray()
    for ch in chars:
        adv = round(f.getlength(ch), 2)
        l, t, r, b = f.getbbox(ch, anchor="ls")
        w, h = max(0, r - l), max(0, b - t)
        if w and h:
            im = Image.new("L", (w, h), 0)
            ImageDraw.Draw(im).text((-l, -t), ch, font=f, fill=255, anchor="ls")
            data = im.tobytes()
        else:
            data = b""
        glyphs[ord(ch)] = [adv, l, t, w, h, len(blob)]
        blob += data
    out[name] = {"asc": asc, "desc": desc, "g": glyphs,
                 "d": base64.b64encode(zlib.compress(bytes(blob), 9)).decode()}

js = ("// Generado por tools/make_font.py (DejaVu Sans Condensed). No editar a mano.\n"
      "'use strict';\nmodule.exports = " + json.dumps(out, separators=(",", ":")) + ";\n")
open("com.malzar.lhmmonitor.sdPlugin/lib/font.js", "w").write(js)
print("font.js", len(js) // 1024, "KB")
