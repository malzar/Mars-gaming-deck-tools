// Lector del data.json de Libre Hardware Monitor y reparto en categorías.
// No depende de nombres concretos de un PC: clasifica por el icono del hardware
// (ImageURL) y por el nombre del grupo de sensores (Load, Temperatures...).
'use strict';

const http = require('http');
const https = require('https');

// ---------------------------------------------------------------- descarga --
function fetchJson(baseUrl, timeoutMs = 2500) {
  return new Promise((resolve, reject) => {
    let url;
    try {
      url = new URL(baseUrl);
      if (!/\/data\.json$/i.test(url.pathname)) {
        url.pathname = url.pathname.replace(/\/?$/, '/') + 'data.json';
      }
    } catch (e) { reject(new Error('Dirección no válida')); return; }

    const mod = url.protocol === 'https:' ? https : http;
    const req = mod.get(url, {
      timeout: timeoutMs,
      rejectUnauthorized: false, // LHM usa un certificado autofirmado
      headers: { Accept: 'application/json' },
    }, (res) => {
      if (res.statusCode !== 200) { res.resume(); reject(new Error(`HTTP ${res.statusCode}`)); return; }
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => {
        try { resolve(JSON.parse(Buffer.concat(chunks).toString('utf8').replace(/^﻿/, ''))); }
        catch (e) { reject(new Error('Respuesta no es JSON')); }
      });
    });
    req.on('timeout', () => req.destroy(new Error('Tiempo de espera agotado')));
    req.on('error', reject);
  });
}

// ------------------------------------------------------------ valores --
// "12,168 V" -> {v: 12.168, u: 'V'} ; "5677,2 MHz" -> {v: 5677.2, u: 'MHz'}
function parseValue(s) {
  if (typeof s === 'number') return { v: s, u: '' };
  if (typeof s !== 'string') return null;
  const m = s.trim().match(/^(-?[\d.,]+)\s*(.*)$/);
  if (!m) return null;
  let num = m[1];
  const hasC = num.includes(','), hasD = num.includes('.');
  if (hasC && hasD) {
    // el último separador es el decimal
    if (num.lastIndexOf(',') > num.lastIndexOf('.')) num = num.replace(/\./g, '').replace(',', '.');
    else num = num.replace(/,/g, '');
  } else if (hasC) num = num.replace(',', '.');
  const v = parseFloat(num);
  return Number.isFinite(v) ? { v, u: m[2].trim() } : null;
}

const RATE = { 'B/s': 1, 'KB/s': 1024, 'MB/s': 1024 ** 2, 'GB/s': 1024 ** 3 };
const SIZE = { MB: 1 / 1024, GB: 1, TB: 1024 };
function toBps(p) { return p ? p.v * (RATE[p.u] || 1) : undefined; }
function toGB(p) { return p ? p.v * (SIZE[p.u] ?? 1) : undefined; }

// ------------------------------------------------------------- árbol --
const GROUPS = {
  load: 'Load', temp: 'Temperatures', clock: 'Clocks', fan: 'Fans', control: 'Controls',
  power: 'Powers', data: 'Data', small: 'SmallData', rate: 'Throughput', volt: 'Voltages',
  level: 'Levels', current: 'Currents', factor: 'Factors', energy: 'Energy',
};
const GROUP_BY_TEXT = Object.fromEntries(Object.entries(GROUPS).map(([k, v]) => [v.toLowerCase(), k]));
const GROUP_BY_IMG = {
  'load.png': 'load', 'temperature.png': 'temp', 'clock.png': 'clock', 'fan.png': 'fan',
  'control.png': 'control', 'throughput.png': 'rate', 'voltage.png': 'volt', 'level.png': 'level',
};
const img = (n) => String(n.ImageURL || '').split('/').pop().toLowerCase();

function hwKind(node) {
  const i = img(node);
  if (i === 'cpu.png') return 'cpu';
  if (['nvidia.png', 'ati.png', 'amd.png', 'intel.png', 'gpu.png'].includes(i)) return 'gpu';
  if (i === 'ram.png') return 'mem';
  if (i === 'hdd.png') return 'disk';
  if (i === 'nic.png') return 'net';
  if (i === 'mainboard.png') return 'board';
  if (i === 'chip.png') return 'chip';
  return null;
}

function groupKind(node) {
  return GROUP_BY_TEXT[String(node.Text || '').toLowerCase()] || GROUP_BY_IMG[img(node)] || null;
}

// Un nodo es grupo de sensores si sus hijos son hojas con valor
const isSensor = (n) => !(n.Children && n.Children.length) && n.Value !== '' && n.Value !== undefined;

// Recorre el árbol y devuelve [{hw, kind, path, parentKind, groups: {kind: [sensor]}}]
function collectHardware(root) {
  const out = [];
  const visit = (node, path, parentHw) => {
    const kind = hwKind(node);
    let hw = null;
    if (kind) {
      hw = { name: String(node.Text || ''), kind, path: path.concat(node.Text).join('/'), parent: parentHw, groups: {} };
      out.push(hw);
    }
    const owner = hw || parentHw;
    for (const c of node.Children || []) {
      if (owner && c.Children && c.Children.length && c.Children.every(isSensor)) {
        const g = groupKind(c);
        if (!g) continue;
        const seen = {};
        const list = owner.groups[g] || (owner.groups[g] = []);
        for (const s of c.Children) {
          const t = String(s.Text || '');
          seen[t] = (seen[t] || 0) + 1;
          list.push({
            name: t,
            id: s.SensorId || `${owner.path}/${GROUPS[g]}/${t}${seen[t] > 1 ? '#' + seen[t] : ''}`,
            value: parseValue(s.Value), min: parseValue(s.Min), max: parseValue(s.Max),
          });
        }
      } else {
        visit(c, path.concat(node.Text), owner);
      }
    }
  };
  visit(root, [], null);
  // Un chip (Super I/O) pertenece a la placa base
  for (const h of out) if (h.kind === 'chip') h.kind = 'board';
  return out;
}

// Utilidades de búsqueda dentro de un hardware
function find(hw, group, re) {
  const list = hw.groups[group] || [];
  return list.find((s) => re.test(s.name));
}
function val(hw, group, re) { const s = find(hw, group, re); return s && s.value ? s.value : undefined; }

// ---------------------------------------------------------- categorías --
// Cada métrica: k, nombre es/en, unidad base, color, umbral por defecto, máximo para barra
const M = (k, es, en, unit, color, o = {}) => ({ k, name: { es, en }, unit, color, ...o });

const CATEGORIES = {
  cpu: {
    name: { es: 'CPU', en: 'CPU' },
    metrics: [
      M('load', 'Carga', 'Load', '%', '#5DCAA5', { th: 90, max: 100 }),
      M('temp', 'Temperatura', 'Temperature', '°C', '#EF9F27', { th: 90, max: 110 }),
      M('freq', 'Frecuencia', 'Frequency', 'GHz', '#85B7EB', {}),
      M('power', 'Consumo', 'Power', 'W', '#F0997B', {}),
    ],
  },
  gpu: {
    name: { es: 'GPU', en: 'GPU' },
    metrics: [
      M('load', 'Carga', 'Load', '%', '#5DCAA5', { th: 95, max: 100 }),
      M('temp', 'Temperatura', 'Temperature', '°C', '#EF9F27', { th: 85, max: 100 }),
      M('hot', 'Hot spot', 'Hot spot', '°C', '#F0997B', { th: 95, max: 110 }),
      M('clk', 'Reloj', 'Clock', 'MHz', '#85B7EB', {}),
      M('vram', 'VRAM', 'VRAM', '%', '#AFA9EC', { th: 95, max: 100 }),
      M('power', 'Consumo', 'Power', 'W', '#ED93B1', {}),
    ],
  },
  mem: {
    name: { es: 'Memoria', en: 'Memory' },
    metrics: [
      M('use', 'Uso', 'Usage', '%', '#5DCAA5', { th: 90, max: 100 }),
      M('used', 'Usada', 'Used', 'GB', '#85B7EB', {}),
      M('free', 'Libre', 'Free', 'GB', '#AFA9EC', {}),
    ],
  },
  disk: {
    name: { es: 'Almacenamiento', en: 'Storage' },
    metrics: [
      M('temp', 'Temperatura', 'Temperature', '°C', '#EF9F27', { th: 70, max: 90 }),
      M('space', 'Espacio usado', 'Used space', '%', '#AFA9EC', { th: 90, max: 100 }),
      M('act', 'Actividad', 'Activity', '%', '#ED93B1', { max: 100 }),
      M('rd', 'Lectura', 'Read', 'B/s', '#5DCAA5', {}),
      M('wr', 'Escritura', 'Write', 'B/s', '#85B7EB', {}),
      M('wear', 'Desgaste', 'Wear', '%', '#F0997B', { th: 80, max: 100 }),
    ],
  },
  net: {
    name: { es: 'Red', en: 'Network' },
    metrics: [
      M('dn', 'Bajada', 'Download', 'B/s', '#5DCAA5', {}),
      M('up', 'Subida', 'Upload', 'B/s', '#85B7EB', {}),
      M('util', 'Utilización', 'Utilization', '%', '#AFA9EC', { th: 90, max: 100 }),
    ],
  },
  fan: {
    name: { es: 'Ventilación', en: 'Cooling' },
    metrics: [
      M('rpm', 'Velocidad', 'Speed', 'RPM', '#85B7EB', {}),
      M('ctl', 'Control', 'Control', '%', '#5DCAA5', { max: 100 }),
    ],
  },
  board: {
    name: { es: 'Placa base', en: 'Motherboard' },
    single: true,
    metrics: [M('val', 'Valor', 'Value', '', '#EF9F27', {})],
  },
};

// Nombres cortos para la tecla
function shortName(s) {
  return String(s)
    .replace(/\((R|TM|C)\)/gi, '')
    .replace(/^(NVIDIA\s+)?GeForce\s+/i, '')
    .replace(/^AMD\s+Radeon\s+/i, '')
    .replace(/^Intel\s+/i, '')
    .replace(/\s+Graphics\b/i, '')
    .replace(/\s+\d+(\.\d+)?\s?(GB|TB)$/i, '')
    .replace(/\s{2,}/g, ' ')
    .trim();
}

const avg = (a) => (a.length ? a.reduce((s, x) => s + x, 0) / a.length : undefined);

function buildCpu(hws) {
  const els = [];
  const cpus = hws.filter((h) => h.kind === 'cpu');
  cpus.forEach((h, ci) => {
    const pre = cpus.length > 1 ? `${ci + 1}·` : '';
    const coreRe = /^(?:CPU\s+)?Core\s*#(\d+)$/i;
    const cores = new Map();
    const get = (n) => { if (!cores.has(n)) cores.set(n, { load: [], threads: 0 }); return cores.get(n); };
    for (const s of h.groups.load || []) {
      const t = s.name.match(/^(?:CPU\s+)?Core\s*#(\d+)(?:\s+Thread\s*#(\d+))?$/i);
      if (t && s.value) { const c = get(+t[1]); c.load.push(s.value.v); c.threads++; }
    }
    for (const s of h.groups.temp || []) { const t = s.name.match(coreRe); if (t && s.value) get(+t[1]).temp = s.value; }
    for (const s of h.groups.clock || []) { const t = s.name.match(coreRe); if (t && s.value) get(+t[1]).freq = s.value.v / 1000; }

    const freqs = [...cores.values()].map((c) => c.freq).filter((x) => x !== undefined);
    const tempTot = val(h, 'temp', /^(CPU Package|Core \(Tctl\/Tdie\)|Core \(Tdie\)|Core \(Tctl\)|Core Average)$/i);
    els.push({
      id: `${h.path}#total`, name: pre + 'Total', tag: '', defaultOn: true,
      values: {
        load: val(h, 'load', /^CPU Total$/i)?.v,
        temp: tempTot?.v, freq: avg(freqs),
        power: val(h, 'power', /^(CPU )?Package$/i)?.v,
      },
      units: { temp: tempTot?.u },
    });
    [...cores.keys()].sort((a, b) => a - b).forEach((n) => {
      const c = cores.get(n);
      els.push({
        id: `${h.path}#core${n}`, name: `${pre}CPU #${n}`, tag: c.threads > 1 ? 'HT' : '', defaultOn: true,
        values: { load: c.load.length ? Math.max(...c.load) : undefined, temp: c.temp?.v, freq: c.freq },
        units: { temp: c.temp?.u },
      });
    });
  });
  return els;
}

function buildGpu(hws) {
  return hws.filter((h) => h.kind === 'gpu').map((h) => {
    const t = val(h, 'temp', /^GPU Core$/i) || val(h, 'temp', /^(GPU )?(Temperature|Edge)$/i);
    const hot = val(h, 'temp', /Hot ?Spot|Junction/i);
    let vram = val(h, 'load', /^GPU Memory$/i)?.v;
    if (vram === undefined) {
      const re = (w) => new RegExp(`^(GPU Memory ${w}|D3D Dedicated Memory ${w}|D3D Shared Memory ${w})$`, 'i');
      const used = toGB(val(h, 'small', /^GPU Memory Used$/i) || val(h, 'data', re('Used')));
      const total = toGB(val(h, 'small', /^GPU Memory Total$/i) || val(h, 'data', re('Total')));
      if (used !== undefined && total) vram = (used / total) * 100;
    }
    return {
      id: h.path, name: shortName(h.name), tag: '', defaultOn: true,
      values: {
        load: (val(h, 'load', /^GPU Core$/i) || val(h, 'load', /^D3D 3D$/i))?.v,
        temp: t?.v, hot: hot?.v,
        clk: val(h, 'clock', /^GPU Core$/i)?.v,
        vram,
        power: val(h, 'power', /^(GPU Package|GPU Power|GPU Core)$/i)?.v,
      },
      units: { temp: t?.u, hot: hot?.u },
    };
  });
}

function buildMem(hws) {
  const els = [];
  for (const h of hws.filter((x) => x.kind === 'mem')) {
    const sets = [
      { key: 'ram', es: 'RAM', load: /^Memory$/i, used: /^(Memory Used|Used Memory)$/i, free: /^(Memory Available|Available Memory)$/i },
      { key: 'virt', es: 'Virtual', load: /^Virtual Memory$/i, used: /^(Virtual Memory Used|Used Virtual Memory)$/i, free: /^(Virtual Memory Available|Available Virtual Memory)$/i },
    ];
    for (const s of sets) {
      const use = val(h, 'load', s.load)?.v;
      const used = toGB(val(h, 'data', s.used));
      const free = toGB(val(h, 'data', s.free));
      if (use === undefined && used === undefined) continue;
      els.push({ id: `${h.path}#${s.key}`, name: s.es, tag: '', defaultOn: true, values: { use, used, free }, units: {} });
    }
  }
  return els;
}

function buildDisk(hws) {
  return hws.filter((h) => h.kind === 'disk').map((h) => {
    const t = val(h, 'temp', /^(Temperature|Composite Temperature)$/i) || (h.groups.temp || [])[0]?.value;
    let wear = val(h, 'level', /^Percentage Used$/i)?.v;
    if (wear === undefined) { const life = val(h, 'level', /^Remaining Life$/i)?.v; if (life !== undefined) wear = 100 - life; }
    return {
      id: h.path, name: shortName(h.name), tag: '', defaultOn: true,
      values: {
        temp: t?.v,
        space: val(h, 'load', /^Used Space$/i)?.v,
        act: val(h, 'load', /^Total Activity$/i)?.v,
        rd: toBps(val(h, 'rate', /^Read Rate$/i)),
        wr: toBps(val(h, 'rate', /^Write Rate$/i)),
        wear,
      },
      units: { temp: t?.u },
    };
  });
}

function buildNet(hws) {
  return hws.filter((h) => h.kind === 'net').map((h) => {
    const dn = toBps(val(h, 'rate', /^Download Speed$/i));
    const up = toBps(val(h, 'rate', /^Upload Speed$/i));
    const totalGB = (toGB(val(h, 'data', /^Data Downloaded$/i)) || 0) + (toGB(val(h, 'data', /^Data Uploaded$/i)) || 0);
    const idle = totalGB === 0 && !dn && !up;
    return {
      id: h.path, name: h.name, tag: '', defaultOn: !idle, hidden: idle ? 'idle' : '',
      values: { dn, up, util: val(h, 'load', /^Network Utilization$/i)?.v },
      units: {},
    };
  });
}

function buildFans(hws) {
  const els = [];
  for (const h of hws) {
    const ctl = h.groups.control || [];
    for (const f of h.groups.fan || []) {
      const c = ctl.find((x) => x.name === f.name);
      const dead = f.max && f.max.v === 0;
      els.push({
        id: f.id,
        name: f.name.replace(/^System Fan/i, 'Sys Fan').replace(/^Pump Fan$/i, 'Pump'),
        tag: '', defaultOn: !dead, hidden: dead ? 'dead' : '',
        values: { rpm: f.value?.v, ctl: c?.value?.v },
        units: {},
      });
    }
  }
  return els;
}

function buildBoard(hws) {
  const els = [];
  for (const h of hws.filter((x) => x.kind === 'board')) {
    for (const s of h.groups.temp || []) {
      els.push({ id: s.id, name: s.name, tag: '', defaultOn: true, unit: s.value?.u || '°C', kind: 'temp',
        values: { val: s.value?.v }, units: { val: s.value?.u } });
    }
    for (const s of h.groups.volt || []) {
      els.push({ id: s.id, name: s.name, tag: '', defaultOn: false, unit: 'V', kind: 'volt',
        values: { val: s.value?.v }, units: { val: s.value?.u } });
    }
  }
  return els;
}

const BUILDERS = { cpu: buildCpu, gpu: buildGpu, mem: buildMem, disk: buildDisk, net: buildNet, fan: buildFans, board: buildBoard };

// Devuelve { cpu: [elementos], gpu: [...], ... } y qué métricas existen en cada categoría
function buildModel(root) {
  const hws = collectHardware(root);
  const model = {};
  for (const [cat, fn] of Object.entries(BUILDERS)) {
    const elements = fn(hws);
    const available = CATEGORIES[cat].metrics
      .filter((m) => elements.some((e) => e.values[m.k] !== undefined))
      .map((m) => m.k);
    model[cat] = { elements, available };
  }
  return model;
}

module.exports = { fetchJson, parseValue, buildModel, CATEGORIES, shortName };
