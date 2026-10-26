// Dibuja la tecla (144×144) en un lienzo propio y la codifica a PNG.
// Sin dependencias nativas: funciona igual en OpenDeck y en Stream Deck de Elgato.
'use strict';

const zlib = require('zlib');
const FONT = require('./font');

const SIZE = 144;
const BG = [14, 17, 22];
const GRAY = '#8a94a3';
const RED = '#E24B4A';
const TRACK = '#2a313c';
const AMBER = '#EF9F27';

// ------------------------------------------------------------ fuente --
const atlas = {};
function font(name) {
  if (!atlas[name]) {
    const f = FONT[name];
    atlas[name] = { ...f, data: zlib.inflateSync(Buffer.from(f.d, 'base64')) };
  }
  return atlas[name];
}
function glyph(f, ch) { return f.g[ch.codePointAt(0)] || f.g[63]; } // '?' si no existe

function measure(fname, text) {
  const f = font(fname);
  let w = 0;
  for (const ch of String(text)) w += glyph(f, ch)[0];
  return w;
}

// ------------------------------------------------------------ lienzo --
function hex(c) {
  const h = c.replace('#', '');
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16)];
}

class Canvas {
  constructor(w = SIZE, h = SIZE, bg = BG) {
    this.w = w; this.h = h;
    this.px = new Uint8Array(w * h * 3);
    for (let i = 0; i < w * h; i++) this.px.set(bg, i * 3);
  }
  blend(x, y, rgb, a) {
    if (x < 0 || y < 0 || x >= this.w || y >= this.h || a <= 0) return;
    const i = (y * this.w + x) * 3;
    if (a >= 1) { this.px[i] = rgb[0]; this.px[i + 1] = rgb[1]; this.px[i + 2] = rgb[2]; return; }
    this.px[i] += (rgb[0] - this.px[i]) * a;
    this.px[i + 1] += (rgb[1] - this.px[i + 1]) * a;
    this.px[i + 2] += (rgb[2] - this.px[i + 2]) * a;
  }
  // Rectángulo redondeado con antialias (4×4 muestras en los bordes)
  roundRect(x, y, w, h, r, color) {
    if (w <= 0 || h <= 0) return;
    const rgb = hex(color);
    r = Math.min(r, w / 2, h / 2);
    const inside = (px, py) => {
      const cx = Math.min(Math.max(px, x + r), x + w - r);
      const cy = Math.min(Math.max(py, y + r), y + h - r);
      return (px - cx) ** 2 + (py - cy) ** 2 <= r * r && px >= x && px <= x + w && py >= y && py <= y + h;
    };
    for (let py = Math.floor(y); py < Math.ceil(y + h); py++) {
      for (let px = Math.floor(x); px < Math.ceil(x + w); px++) {
        let hit = 0;
        for (let sy = 0; sy < 4; sy++) for (let sx = 0; sx < 4; sx++) {
          if (inside(px + (sx + 0.5) / 4, py + (sy + 0.5) / 4)) hit++;
        }
        this.blend(px, py, rgb, hit / 16);
      }
    }
  }
  // Texto con la línea base en y. Devuelve el ancho dibujado.
  text(fname, x, y, color, text) {
    const f = font(fname), rgb = hex(color);
    let pen = x;
    for (const ch of String(text)) {
      const [adv, gx, gy, gw, gh, off] = glyph(f, ch);
      const ox = Math.round(pen + gx), oy = Math.round(y + gy);
      for (let j = 0; j < gh; j++) {
        for (let i = 0; i < gw; i++) {
          const a = f.data[off + j * gw + i];
          if (a) this.blend(ox + i, oy + j, rgb, a / 255);
        }
      }
      pen += adv;
    }
    return pen - x;
  }
  // Recorta con "…" si no cabe
  textFit(fname, x, y, color, text, maxW) {
    let t = String(text);
    if (measure(fname, t) > maxW) {
      while (t.length > 1 && measure(fname, t + '…') > maxW) t = t.slice(0, -1);
      t = t.trimEnd() + '…';
    }
    return this.text(fname, x, y, color, t);
  }
  png() { return encodePng(this.w, this.h, this.px); }
}

// ------------------------------------------------------------- PNG --
const CRC = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; }
  return t;
})();
function crc32(buf) { let c = 0xffffffff; for (const b of buf) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; }
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
function encodePng(w, h, rgb) {
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 3 + 1)] = 0;
    Buffer.from(rgb.buffer, rgb.byteOffset + y * w * 3, w * 3).copy(raw, y * (w * 3 + 1) + 1);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw, { level: 6 })), chunk('IEND', Buffer.alloc(0)),
  ]);
}

// --------------------------------------------------------- formato --
// Devuelve [texto, unidad]
function fmt(metric, v, unit, lang = 'es') {
  unit = unit || metric.unit;
  if (v === undefined || v === null || !Number.isFinite(v)) return ['—', unit === 'B/s' ? 'MB/s' : unit];
  const dec = (x, d) => { const s = x.toFixed(d); return lang === 'es' ? s.replace('.', ',') : s; };
  switch (unit) {
    case 'B/s': {
      const u = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
      let i = 0; while (v >= 1024 && i < 3) { v /= 1024; i++; }
      return [i === 0 ? String(Math.round(v)) : dec(v, v < 10 ? 1 : 0), u[i]];
    }
    case 'GHz': return [dec(v, 2), unit];
    case 'GB': return [v >= 100 ? String(Math.round(v)) : dec(v, 1), unit];
    case 'W': return [v < 10 ? dec(v, 1) : String(Math.round(v)), unit];
    case 'V': return [dec(v, v >= 10 ? 2 : 3), unit];
    case '°C': case '°F': return [metric.k === 'val' ? dec(v, 1) : String(Math.round(v)), unit];
    default: return [String(Math.round(v)), unit];
  }
}

function colorFor(metric, v, cfg) {
  const th = cfg.thresholds?.[metric.k];
  if (th !== undefined && th !== null && th !== '' && Number.isFinite(v) && v >= Number(th)) return RED;
  return cfg.colors?.[metric.k] || metric.color;
}

// ----------------------------------------------------------- tecla --
// Diseñada para leerse en teclas pequeñas (85 px físicos en el MSD-ONE):
// lienzo de 144 px, textos de 16 px como mínimo.
const SHORT = { '°C': '°', '°F': '°F', GHz: 'G', MHz: 'M', RPM: '', 'B/s': '', 'KB/s': 'K', 'MB/s': 'M', 'GB/s': 'G', GB: 'G', W: 'W', V: 'V', '%': '%' };

// Escribe el texto en la primera fuente que quepa
function fitFont(fonts, text, maxW) {
  for (const f of fonts) if (measure(f, text) <= maxW) return f;
  return fonts[fonts.length - 1];
}

// opts: { title, pos, total, paused, mode ('all' | clave), metrics:[def], values, units, cfg, lang, label }
function renderKey(o) {
  const c = new Canvas();
  const L = 10, R = SIZE - 10, W = R - L;
  const unitOf = (m) => o.units?.[m.k] || m.unit;

  // Cabecera: nombre (+ pausa). La posición solo si cabe sin recortar el nombre.
  const posTxt = o.total > 1 ? `${o.pos + 1}/${o.total}` : '';
  const pauseW = o.paused ? 14 : 0;
  const titleW = measure('b22', o.title);
  const posW = posTxt ? measure('r16', posTxt) : 0;
  const showPos = posTxt && titleW + pauseW + posW + 8 <= W;
  const tw = c.textFit('b22', L, 28, '#e8edf2', o.title, W - pauseW - (showPos ? posW + 8 : 0));
  if (o.paused) { c.roundRect(L + tw + 5, 11, 4, 17, 1, AMBER); c.roundRect(L + tw + 12, 11, 4, 17, 1, AMBER); }
  if (showPos) c.text('r16', R - posW, 27, GRAY, posTxt);

  if (o.mode === 'all') {
    const [p, ...rest] = o.metrics;
    const v = o.values[p.k];
    const [t, u] = fmt(p, v, unitOf(p), o.lang);
    const col = v === undefined ? GRAY : colorFor(p, v, o.cfg);
    const uw = measure('r18', u);
    const big = fitFont(['b48', 'b40', 'b32'], t, W - uw - 4);
    const w = c.text(big, L, 78, col, t);
    if (w + uw + 3 <= W) c.text('r18', L + w + 3, 78, GRAY, u);
    if (p.max && v !== undefined) {
      c.roundRect(L, 86, W, 7, 3.5, TRACK);
      c.roundRect(L, 86, W * Math.max(0, Math.min(1, v / p.max)), 7, 3.5, col);
    }
    // Secundarios en la fila de abajo, con unidad corta del mismo color
    const items = rest.map((m) => {
      const x = o.values[m.k];
      const [tt, uu] = fmt(m, x, unitOf(m), o.lang);
      const su = SHORT[uu] ?? uu;
      return { text: tt + (tt === '—' ? '' : su), col: x === undefined ? GRAY : colorFor(m, x, o.cfg) };
    });
    if (items.length) {
      const all = items.map((i) => i.text).join('  ');
      const f = fitFont(['b24', 'b22', 'b20'], all, W);
      if (items.length === 1) c.text(f, L, 126, items[0].col, items[0].text);
      else {
        c.text(f, L, 126, items[0].col, items[0].text);
        const rw = measure(f, items[1].text);
        c.text(f, R - rw, 126, items[1].col, items[1].text);
      }
    }
  } else {
    const m = o.metrics.find((x) => x.k === o.mode) || o.metrics[0];
    const v = o.values[m.k];
    const [t, u] = fmt(m, v, unitOf(m), o.lang);
    const col = v === undefined ? GRAY : colorFor(m, v, o.cfg);
    const uw = measure('r20', u);
    const label = o.label || m.name[o.lang] || m.name.es;
    // Si número + unidad no caben en una línea, la unidad baja junto al nombre del dato
    const unitBelow = u && measure('b40', t) + uw + 4 > W;
    const big = fitFont(['b56', 'b48', 'b40', 'b32'], t, unitBelow ? W : W - uw - 4);
    const w = c.text(big, L, 88, col, t);
    if (!unitBelow) c.text('r20', L + w + 4, 88, GRAY, u);
    c.textFit('r18', L, 113, GRAY, unitBelow ? `${u} · ${label}` : label, W);
    if (m.max && v !== undefined) {
      c.roundRect(L, 123, W, 9, 4.5, TRACK);
      c.roundRect(L, 123, W * Math.max(0, Math.min(1, v / m.max)), 9, 4.5, col);
    }
  }
  return c.png();
}

// Tecla de aviso (sin conexión, sin datos...)
function renderMessage(title, lines, accent = AMBER) {
  const c = new Canvas();
  c.textFit('b22', 10, 30, '#e8edf2', title, 124);
  c.roundRect(10, 40, 30, 4, 2, accent);
  // Una dirección larga (ip:puerto) se parte en dos líneas
  lines = lines.flatMap((l, i) => (i > 0 && measure('r16', l) > 124 && l.includes(':') ? [l.slice(0, l.lastIndexOf(':')), l.slice(l.lastIndexOf(':'))] : [l]));
  lines.forEach((l, i) => {
    const f = i === 0 ? fitFont(['b20', 'r18', 'r16'], l, 124) : fitFont(['r16'], l, 124);
    c.textFit(f, 10, 76 + i * 24, i === 0 ? '#e8edf2' : GRAY, l, 124);
  });
  return c.png();
}

module.exports = { renderKey, renderMessage, fmt, Canvas, measure };
