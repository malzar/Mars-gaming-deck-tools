// LHMonitor fancy — plugin para OpenDeck y Stream Deck.
// Muestra datos de Libre Hardware Monitor en una tecla, rotando entre elementos
// (núcleos, GPUs, ventiladores...). 1 clic: modo · 2 clics: pausa · 3 clics: siguiente.
'use strict';

const fs = require('fs');
const path = require('path');
const http = require('http');
const { spawn } = require('child_process');
const MiniWS = require('./lib/ws');
const { fetchJson, buildModel, CATEGORIES } = require('./lib/lhm');
const { renderKey, renderMessage } = require('./lib/render');

// ------------------------------------------------------------ arranque --
const args = {};
for (let i = 2; i < process.argv.length; i += 2) args[String(process.argv[i]).replace(/^-+/, '')] = process.argv[i + 1];
let info = {};
try { info = JSON.parse(args.info || '{}'); } catch (_) {}
const sysLang = String(info?.application?.language || Intl.DateTimeFormat().resolvedOptions().locale || 'en');
const LANG = sysLang.toLowerCase().startsWith('es') ? 'es' : 'en';

const T = {
  es: { connecting: 'Conectando…', offline: 'Sin conexión', noData: 'Sin datos', noEls: 'Sin elementos', openPanel: 'Abre el panel', temp: 'Temperatura', volt: 'Voltaje' },
  en: { connecting: 'Connecting…', offline: 'Offline', noData: 'No data', noEls: 'No items', openPanel: 'Open the panel', temp: 'Temperature', volt: 'Voltage' },
}[LANG];

const LOG = path.join(__dirname, 'plugin.log');
function log(msg) {
  const line = `${new Date().toISOString()}  ${msg}`;
  try {
    if (fs.existsSync(LOG) && fs.statSync(LOG).size > 512 * 1024) fs.renameSync(LOG, LOG + '.old');
    fs.appendFileSync(LOG, line + '\n');
  } catch (_) {}
  send({ event: 'logMessage', payload: { message: `[lhm] ${msg}` } });
}

// ------------------------------------------------------------- estado --
const DEFAULT_URL = 'http://localhost:8085';
let globalSettings = { url: DEFAULT_URL };
let model = null;            // último modelo de LHM
let modelVersion = 0;
let lhmError = '';
const contexts = new Map();  // context -> { action, category, settings, ... }

const DEFAULT_METRICS = {
  cpu: ['load', 'temp', 'freq'], gpu: ['load', 'temp', 'clk'], mem: ['use', 'used', 'free'],
  disk: ['temp', 'rd', 'wr'], net: ['dn', 'up'], fan: ['rpm', 'ctl'], board: ['val'],
};
const ACTION_CAT = (uuid) => {
  const k = String(uuid).split('.').pop();
  return CATEGORIES[k] ? k : 'cpu';
};

function catDefaults(cat) {
  const avail = model?.[cat]?.available;
  let metrics = DEFAULT_METRICS[cat].filter((k) => !avail || avail.includes(k));
  if (!metrics.length && avail?.length) metrics = avail.slice(0, 1);
  return { metrics, combined: metrics.length > 1, onlyCombined: false, colors: {}, thresholds: {}, order: [], enabled: {}, names: {} };
}

// Completa ajustes que falten (tecla nueva o versión anterior)
function normalize(settings, actionCat) {
  const s = JSON.parse(JSON.stringify(settings || {}));
  s.category = CATEGORIES[s.category] ? s.category : actionCat;
  s.rotateSec = clamp(+s.rotateSec || 3, 1, 3600);
  s.refreshMs = clamp(+s.refreshMs || 1000, 250, 10000);
  s.clickMs = clamp(+s.clickMs || 350, 150, 1000);
  s.cats = s.cats || {};
  for (const cat of Object.keys(CATEGORIES)) {
    const d = catDefaults(cat);
    const c = (s.cats[cat] = { ...d, ...(s.cats[cat] || {}) });
    if (!Array.isArray(c.metrics) || !c.metrics.length) c.metrics = d.metrics;
    if (CATEGORIES[cat].single) { c.metrics = ['val']; c.combined = false; c.onlyCombined = false; }
    c.metrics = c.metrics.slice(0, 3);
  }
  s.state = { mode: 0, paused: false, pos: 0, ...(s.state || {}) };
  return s;
}
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

function modesOf(c) {
  if (c.metrics.length >= 2 && c.onlyCombined) return ['all'];
  const m = [...c.metrics];
  if (c.metrics.length >= 2 && c.combined) m.push('all');
  return m;
}

function elementsOf(cat, c) {
  const els = model?.[cat]?.elements || [];
  const byId = new Map(els.map((e) => [e.id, e]));
  const order = c.order.filter((id) => byId.has(id));
  for (const e of els) if (!order.includes(e.id)) order.push(e.id);
  return order.map((id) => byId.get(id)).filter((e) => (c.enabled[e.id] ?? e.defaultOn));
}

// ------------------------------------------------------------ dibujo --
function buildImage(ctx, override) {
  const s = override?.settings || ctx.settings;
  const st = override?.state || s.state;
  const cat = s.category;
  const c = s.cats[cat];
  const catName = CATEGORIES[cat].name[LANG];
  if (!model) {
    return renderMessage(catName, lhmError ? [T.offline, shortUrl(globalSettings.url)] : [T.connecting], lhmError ? '#E24B4A' : '#EF9F27');
  }
  const els = elementsOf(cat, c);
  if (!els.length) return renderMessage(catName, [T.noEls, T.openPanel]);
  const pos = ((st.pos % els.length) + els.length) % els.length;
  const e = els[pos];
  const modes = modesOf(c);
  const mode = modes[((st.mode % modes.length) + modes.length) % modes.length];
  const defs = c.metrics.map((k) => CATEGORIES[cat].metrics.find((m) => m.k === k)).filter(Boolean);
  const units = { ...(e.units || {}) };
  for (const k in units) if (!units[k]) delete units[k];
  return renderKey({
    title: c.names[e.id] || e.name,
    pos, total: els.length, paused: st.paused, mode, metrics: defs, values: e.values, units,
    cfg: { colors: c.colors, thresholds: c.thresholds },
    label: cat === 'board' ? (e.kind === 'volt' ? T.volt : T.temp) : undefined,
    lang: LANG,
  });
}
const shortUrl = (u) => String(u).replace(/^https?:\/\//, '').replace(/\/.*$/, '');

function paint(ctx, force) {
  const png = buildImage(ctx);
  const uri = 'data:image/png;base64,' + png.toString('base64');
  if (!force && uri === ctx.lastUri) return;
  ctx.lastUri = uri;
  send({ event: 'setImage', context: ctx.id, payload: { image: uri, target: 0 } });
}

// ----------------------------------------------------------- sondeo LHM --
let polling = false;
async function poll() {
  if (polling) return;
  polling = true;
  try {
    const data = await fetchJson(globalSettings.url);
    model = buildModel(data);
    modelVersion++;
    if (lhmError) log(`LHM conectado (${globalSettings.url})`);
    lhmError = '';
  } catch (e) {
    if (lhmError !== e.message) log(`LHM: ${e.message} (${globalSettings.url})`);
    lhmError = e.message;
    model = null;
    modelVersion++;
  } finally { polling = false; }
}

let lastPoll = 0;
setInterval(() => {
  const now = Date.now();
  if (!contexts.size && !panelOpen()) return;
  const every = Math.min(...[...contexts.values()].map((c) => c.settings.refreshMs), 1000);
  if (now - lastPoll >= every) { lastPoll = now; poll(); }

  for (const ctx of contexts.values()) {
    const s = ctx.settings, st = s.state;
    let dirty = ctx.dirty;
    if (!st.paused && now - ctx.lastRot >= s.rotateSec * 1000) {
      const n = model ? elementsOf(s.category, s.cats[s.category]).length : 0;
      if (n > 1) { st.pos = (st.pos + 1) % n; dirty = true; }
      ctx.lastRot = now;
    }
    if (ctx.seenVersion !== modelVersion && now - ctx.lastPaint >= s.refreshMs) { ctx.seenVersion = modelVersion; dirty = true; }
    if (dirty) { ctx.dirty = false; ctx.lastPaint = now; paint(ctx); }
  }
}, 100);

// ------------------------------------------------------- pulsaciones --
function onKeyDown(ctx) {
  ctx.clicks = (ctx.clicks || 0) + 1;
  clearTimeout(ctx.clickTimer);
  ctx.clickTimer = setTimeout(() => {
    const n = ctx.clicks; ctx.clicks = 0;
    const s = ctx.settings, st = s.state, c = s.cats[s.category];
    if (n === 1) {
      const modes = modesOf(c);
      if (modes.length > 1) st.mode = (st.mode + 1) % modes.length;
      else return; // un solo modo: no hace nada
    } else if (n === 2) {
      st.paused = !st.paused;
    } else {
      const total = model ? elementsOf(s.category, c).length : 0;
      if (total > 1) st.pos = (st.pos + 1) % total;
      ctx.lastRot = Date.now();
    }
    ctx.dirty = true;
    saveState(ctx);
  }, ctx.settings.clickMs);
}

function saveState(ctx) {
  clearTimeout(ctx.saveTimer);
  ctx.saveTimer = setTimeout(() => send({ event: 'setSettings', context: ctx.id, payload: ctx.settings }), 500);
}

// ------------------------------------------------- conexión con la app --
let ws = null;
function send(obj) { if (ws && ws.readyState === 1) ws.send(JSON.stringify(obj)); }

function addContext(msg) {
  const cat = ACTION_CAT(msg.action);
  const ctx = {
    id: msg.context, action: msg.action, device: msg.device,
    settings: normalize(msg.payload?.settings, cat),
    lastRot: Date.now(), lastPaint: 0, seenVersion: -1, dirty: true, lastUri: '',
  };
  contexts.set(msg.context, ctx);
  if (!msg.payload?.settings?.cats) send({ event: 'setSettings', context: ctx.id, payload: ctx.settings });
  paint(ctx, true);
  if (!lastPoll) { lastPoll = Date.now(); poll(); }
}

function connect() {
  if (!args.port) { log('Sin puerto de conexión: este archivo lo arranca OpenDeck o Stream Deck.'); return; }
  ws = new MiniWS(`ws://127.0.0.1:${args.port}`);
  ws.on('open', () => {
    ws.send(JSON.stringify({ event: args.registerEvent, uuid: args.pluginUUID }));
    send({ event: 'getGlobalSettings', context: args.pluginUUID });
    log(`Conectado (idioma: ${LANG})`);
  });
  ws.on('message', (raw) => {
    let msg;
    try { msg = JSON.parse(raw); } catch (_) { return; }
    const ctx = contexts.get(msg.context);
    switch (msg.event) {
      case 'didReceiveGlobalSettings':
        globalSettings = { url: DEFAULT_URL, ...(msg.payload?.settings || {}) };
        lastPoll = 0; // fuerza lectura con la nueva dirección
        break;
      case 'willAppear': addContext(msg); break;
      case 'willDisappear': if (ctx) { clearTimeout(ctx.clickTimer); contexts.delete(msg.context); } break;
      case 'didReceiveSettings':
        if (ctx) { ctx.settings = normalize(msg.payload?.settings, ACTION_CAT(ctx.action)); ctx.dirty = true; }
        break;
      case 'keyDown': if (ctx) onKeyDown(ctx); break;
      case 'propertyInspectorDidAppear':
        if (ctx) sendStatus(ctx);
        break;
      case 'sendToPlugin':
        if (ctx && msg.payload?.cmd === 'openPanel') openPanel(ctx.id);
        if (ctx && msg.payload?.cmd === 'status') sendStatus(ctx);
        break;
    }
  });
  ws.on('close', () => process.exit(0));
  ws.on('error', (e) => log(`WebSocket: ${e.message}`));
}

function sendStatus(ctx) {
  send({
    event: 'sendToPropertyInspector', context: ctx.id,
    payload: { lang: LANG, url: globalSettings.url, ok: !!model, error: lhmError, category: ctx.settings.category },
  });
}

// --------------------------------------------------------------- panel --
let serverPort = 0;
let panelSeen = 0;
const panelOpen = () => Date.now() - panelSeen < 10000;
const PANEL = path.join(__dirname, 'panel', 'index.html');

function json(res, code, obj) {
  res.writeHead(code, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
  res.end(JSON.stringify(obj));
}
function readBody(req, limit = 1024 * 1024) {
  return new Promise((resolve, reject) => {
    const chunks = []; let n = 0;
    req.on('data', (c) => { n += c.length; if (n > limit) { reject(new Error('demasiado grande')); req.destroy(); } else chunks.push(c); });
    req.on('end', () => { try { resolve(JSON.parse(Buffer.concat(chunks).toString('utf8') || '{}')); } catch (e) { reject(e); } });
    req.on('error', reject);
  });
}

function catalog() {
  const out = {};
  for (const [cat, def] of Object.entries(CATEGORIES)) {
    const m = model?.[cat];
    out[cat] = {
      name: def.name[LANG], single: !!def.single,
      metrics: def.metrics.map((x) => ({
        k: x.k, name: x.name[LANG], unit: x.unit === 'B/s' ? 'MB/s' : x.unit, color: x.color,
        th: x.th, available: !m || m.available.includes(x.k),
      })),
      elements: (m?.elements || []).map((e) => ({
        id: e.id, name: e.name, tag: e.tag, hidden: e.hidden || '', defaultOn: e.defaultOn, unit: e.unit || '',
      })),
    };
  }
  return out;
}

const server = http.createServer(async (req, res) => {
  try {
    const url = new URL(req.url, 'http://127.0.0.1');
    if (req.method === 'POST' && req.headers.origin && req.headers.origin !== `http://127.0.0.1:${serverPort}`) {
      return json(res, 403, { error: 'origen no permitido' });
    }
    panelSeen = Date.now();
    const ctx = contexts.get(url.searchParams.get('ctx') || '');

    if (req.method === 'GET' && url.pathname === '/') {
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' });
      return res.end(fs.readFileSync(PANEL));
    }
    if (url.pathname.startsWith('/api/') && !ctx) return json(res, 404, { error: LANG === 'es' ? 'Tecla no encontrada. Vuelve a abrir el panel desde OpenDeck.' : 'Key not found. Reopen the panel from OpenDeck.' });

    if (req.method === 'GET' && url.pathname === '/api/state') {
      return json(res, 200, {
        lang: LANG, url: globalSettings.url, connected: !!model, error: lhmError,
        settings: ctx.settings, catalog: catalog(),
      });
    }
    if (req.method === 'POST' && url.pathname === '/api/preview') {
      const body = await readBody(req);
      const settings = normalize(body.settings, ctx.settings.category);
      const state = { ...settings.state, ...(body.state || {}) };
      const c = settings.cats[settings.category];
      const total = model ? elementsOf(settings.category, c).length : 0;
      const png = buildImage(ctx, { settings, state });
      return json(res, 200, { png: 'data:image/png;base64,' + png.toString('base64'), modes: modesOf(c), total, connected: !!model });
    }
    if (req.method === 'POST' && url.pathname === '/api/save') {
      const body = await readBody(req);
      ctx.settings = normalize(body.settings, ACTION_CAT(ctx.action));
      ctx.dirty = true; ctx.lastRot = Date.now();
      send({ event: 'setSettings', context: ctx.id, payload: ctx.settings });
      if (body.url && body.url !== globalSettings.url) {
        globalSettings = { ...globalSettings, url: body.url };
        send({ event: 'setGlobalSettings', context: args.pluginUUID, payload: globalSettings });
        lastPoll = 0;
      }
      return json(res, 200, { ok: true });
    }
    if (req.method === 'POST' && url.pathname === '/api/test') {
      const body = await readBody(req);
      try {
        const m = buildModel(await fetchJson(body.url));
        const counts = Object.fromEntries(Object.entries(m).map(([k, v]) => [k, v.elements.length]));
        // Si conecta, se adopta esa dirección para todas las teclas
        if (body.url !== globalSettings.url) {
          globalSettings = { ...globalSettings, url: body.url };
          send({ event: 'setGlobalSettings', context: args.pluginUUID, payload: globalSettings });
          log(`Dirección de LHM: ${body.url}`);
        }
        model = m; modelVersion++; lhmError = ''; lastPoll = Date.now();
        return json(res, 200, { ok: true, counts });
      } catch (e) { return json(res, 200, { ok: false, error: e.message }); }
    }
    res.writeHead(404); res.end();
  } catch (e) {
    log(`Panel: ${e.message}`);
    json(res, 500, { error: e.message });
  }
});
server.listen(0, '127.0.0.1', () => { serverPort = server.address().port; });

function openPanel(ctxId) {
  const url = `http://127.0.0.1:${serverPort}/?ctx=${encodeURIComponent(ctxId)}`;
  panelSeen = Date.now();
  if (process.platform !== 'win32') { send({ event: 'openUrl', payload: { url } }); return; }
  const opts = { detached: true, stdio: 'ignore', windowsHide: true, windowsVerbatimArguments: true };
  const p = spawn('cmd.exe', ['/c', `start "" msedge --app="${url}" --window-size=760,860`], opts);
  p.on('exit', (code) => { if (code !== 0) send({ event: 'openUrl', payload: { url } }); });
  p.on('error', () => send({ event: 'openUrl', payload: { url } }));
  p.unref();
}

connect();
