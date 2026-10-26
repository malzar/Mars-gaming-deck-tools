// Gestor de packs de iconos para OpenDeck (SDK Stream Deck, Node.js 22+)
// - Packs disponibles:  %USERPROFILE%\StreamDeck\IconPacks\<pack>\  (caché con las imágenes)
// - Packs instalados:   %USERPROFILE%\StreamDeck\Icons\<pack>\
// - La tecla abre una ventana (Edge en modo app) servida en 127.0.0.1 para instalar/desinstalar.
// - Vigila Descargas: cada .streamDeckIconPack nuevo se añade como "Disponible".
'use strict';

const fs = require('fs');
const path = require('path');
const os = require('os');
const http = require('http');
const { execFile, spawn } = require('child_process');

// ---------- Configuración ----------
const BASE = path.join(os.homedir(), 'StreamDeck');
const ICONS = path.join(BASE, 'Icons');          // instalados
const PACKS = path.join(BASE, 'IconPacks');      // disponibles (caché)
const ORIG = path.join(PACKS, '_originales');    // archivos originales
const WATCH = path.join(os.homedir(), 'Downloads');
const LOG = path.join(PACKS, '_registro.log');
const UI_FILE = path.join(__dirname, 'ui.html');
const IMG_RE = /\.(png|jpe?g|gif|svg|webp)$/i;
const PACK_RE = /\.(streamdeckiconpack|zip)$/i;
for (const d of [ICONS, PACKS, ORIG]) fs.mkdirSync(d, { recursive: true });

// ---------- Argumentos que pasa OpenDeck ----------
const args = {};
for (let i = 2; i < process.argv.length; i += 2) {
  args[String(process.argv[i]).replace(/^-+/, '')] = process.argv[i + 1];
}

// ---------- Log ----------
let ws = null;
function log(msg) {
  const line = `${new Date().toISOString().replace('T', ' ').slice(0, 19)}  ${msg}`;
  try { fs.appendFileSync(LOG, line + '\n', 'utf8'); } catch (_) {}
  send({ event: 'logMessage', payload: { message: `[iconpacks] ${msg}` } });
}

// ---------- Utilidades ----------
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const psQuote = (s) => `'${String(s).replace(/'/g, "''")}'`;
const safeName = (s) => String(s).replace(/[\\/:*?"<>|]/g, '_').replace(/\.+$/, '').trim();

function listDirs(dir) {
  try {
    return fs.readdirSync(dir, { withFileTypes: true })
      .filter((e) => e.isDirectory() && !e.name.startsWith('_') && !e.name.startsWith('.'))
      .map((e) => e.name);
  } catch (_) { return []; }
}
function listImages(dir) {
  try { return fs.readdirSync(dir).filter((f) => IMG_RE.test(f)).sort(); } catch (_) { return []; }
}
// Solo acepta un nombre que exista de verdad en esa carpeta (evita rutas tipo ..\)
function resolveIn(dir, name) {
  return listDirs(dir).includes(name) ? path.join(dir, name) : null;
}
function findAll(dir, test, acc = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (test(e)) acc.push(p);
    if (e.isDirectory()) findAll(p, test, acc);
  }
  return acc;
}
function expandArchive(zip, dest) {
  return new Promise((resolve, reject) => {
    execFile('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
      `Expand-Archive -LiteralPath ${psQuote(zip)} -DestinationPath ${psQuote(dest)} -Force`],
      { windowsHide: true }, (err, _o, stderr) => (err ? reject(new Error(stderr || err.message)) : resolve()));
  });
}
async function waitReady(file) {
  let last = -1;
  for (let i = 0; i < 120; i++) {
    try {
      const size = fs.statSync(file).size;
      fs.closeSync(fs.openSync(file, 'r+'));
      if (size > 0 && size === last) return true;
      last = size;
    } catch (_) {}
    await sleep(500);
  }
  return false;
}
function moveFile(src, dst) {
  try { fs.renameSync(src, dst); } catch (_) { fs.copyFileSync(src, dst); fs.unlinkSync(src); }
}
function uniqueDir(parent, name) {
  let n = name, i = 2;
  while (fs.existsSync(path.join(parent, n))) n = `${name} (${i++})`;
  return n;
}

// ---------- Packs ----------
function packName(dir) {
  try { return JSON.parse(fs.readFileSync(path.join(dir, 'pack.json'), 'utf8')).name || path.basename(dir); }
  catch (_) { return path.basename(dir); }
}

// Hasta n imágenes repartidas por todo el pack (para sacar su paleta)
function spread(list, n) {
  if (list.length <= n) return list;
  return Array.from({ length: n }, (_, i) => list[Math.floor((i * list.length) / n)]);
}

// Lista combinada: disponibles (caché) + instalados que no estén en caché
function getPacks() {
  const out = [];
  const installed = new Set(listDirs(ICONS));
  const seen = new Set();
  for (const d of listDirs(PACKS)) {
    const dir = path.join(PACKS, d);
    const name = packName(dir);
    const imgs = listImages(path.join(dir, 'icons'));
    out.push({
      id: `c:${d}`, name, count: imgs.length, installed: installed.has(name),
      thumbs: imgs.slice(0, 3).map((f) => `/thumb/c/${encodeURIComponent(d)}/${encodeURIComponent(f)}`),
      samples: spread(imgs, 12).map((f) => `/thumb/c/${encodeURIComponent(d)}/${encodeURIComponent(f)}`),
    });
    seen.add(name);
  }
  for (const d of installed) {
    if (seen.has(d)) continue;
    const imgs = listImages(path.join(ICONS, d));
    out.push({
      id: `i:${d}`, name: d, count: imgs.length, installed: true, onlyInstalled: true,
      thumbs: imgs.slice(0, 3).map((f) => `/thumb/i/${encodeURIComponent(d)}/${encodeURIComponent(f)}`),
      samples: spread(imgs, 12).map((f) => `/thumb/i/${encodeURIComponent(d)}/${encodeURIComponent(f)}`),
    });
  }
  return out.sort((a, b) => a.name.localeCompare(b.name, 'es'));
}

// Añade un archivo de pack a la caché (no lo instala)
async function addPack(file, { keepOriginal = true } = {}) {
  const base = path.basename(file).replace(PACK_RE, '');
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'iconpack_'));
  const zip = tmp + '.zip';
  try {
    fs.copyFileSync(file, zip);
    await expandArchive(zip, tmp);
    const iconDirs = findAll(tmp, (e) => e.isDirectory() && e.name.toLowerCase() === 'icons');
    if (!iconDirs.length) throw new Error('el archivo no contiene una carpeta "icons"');

    let name = base;
    const manifest = findAll(tmp, (e) => e.isFile() && e.name.toLowerCase() === 'manifest.json')[0];
    if (manifest) {
      try {
        const m = JSON.parse(fs.readFileSync(manifest, 'utf8').replace(/^﻿/, ''));
        if (m.Name) name = String(m.Name);
      } catch (_) {}
    }
    name = safeName(name) || base;

    // Si ya existe un pack con ese nombre, se reemplaza su caché
    const existing = listDirs(PACKS).find((d) => packName(path.join(PACKS, d)) === name);
    const dirName = existing || uniqueDir(PACKS, name);
    const dest = path.join(PACKS, dirName);
    fs.rmSync(dest, { recursive: true, force: true });
    fs.mkdirSync(path.join(dest, 'icons'), { recursive: true });
    let n = 0;
    for (const d of iconDirs) {
      for (const f of fs.readdirSync(d)) {
        if (IMG_RE.test(f)) { fs.copyFileSync(path.join(d, f), path.join(dest, 'icons', f)); n++; }
      }
    }
    fs.writeFileSync(path.join(dest, 'pack.json'),
      JSON.stringify({ name, source: path.basename(file), added: new Date().toISOString() }, null, 2));
    if (keepOriginal) moveFile(file, path.join(ORIG, path.basename(file)));
    log(`Añadido '${name}' (${n} iconos)`);
    return { name, count: n };
  } finally {
    fs.rmSync(zip, { force: true });
    fs.rmSync(tmp, { recursive: true, force: true });
  }
}

function installPack(id) {
  if (!id.startsWith('c:')) throw new Error('pack no válido');
  const src = resolveIn(PACKS, id.slice(2));
  if (!src) throw new Error('pack no encontrado');
  const name = packName(src);
  const dest = path.join(ICONS, safeName(name));
  fs.mkdirSync(dest, { recursive: true });
  const imgs = listImages(path.join(src, 'icons'));
  for (const f of imgs) fs.copyFileSync(path.join(src, 'icons', f), path.join(dest, f));
  log(`Instalado '${name}' en ${dest}`);
}

function uninstallPack(id) {
  let name;
  if (id.startsWith('c:')) {
    const src = resolveIn(PACKS, id.slice(2));
    if (!src) throw new Error('pack no encontrado');
    name = safeName(packName(src));
  } else if (id.startsWith('i:')) {
    name = id.slice(2);
  } else throw new Error('pack no válido');
  const dir = resolveIn(ICONS, name);
  if (!dir) throw new Error('no está instalado');
  fs.rmSync(dir, { recursive: true, force: true });
  log(`Desinstalado '${name}'`);
}

// ---------- Cola de importación desde Descargas ----------
const pending = new Set();
let chain = Promise.resolve();
function enqueue(file) {
  const key = file.toLowerCase();
  if (pending.has(key)) return chain;
  pending.add(key);
  chain = chain
    .then(() => sleep(1000))
    .then(async () => {
      if (!fs.existsSync(file)) return;
      if (!(await waitReady(file))) { log(`No se pudo abrir: ${file}`); return; }
      await addPack(file);
      updateTitles(); flashOk();
    })
    .catch((e) => log(`Error con ${path.basename(file)}: ${e.message}`))
    .finally(() => pending.delete(key));
  return chain;
}
function scanDownloads() {
  try {
    for (const f of fs.readdirSync(WATCH)) if (/\.streamdeckiconpack$/i.test(f)) enqueue(path.join(WATCH, f));
  } catch (e) { log(`No se puede leer ${WATCH}: ${e.message}`); }
}

// ---------- Servidor local de la ventana ----------
const MIME = { '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.gif': 'image/gif',
  '.svg': 'image/svg+xml', '.webp': 'image/webp' };

function readBody(req, limit = 200 * 1024 * 1024) {
  return new Promise((resolve, reject) => {
    const chunks = []; let size = 0;
    req.on('data', (c) => { size += c.length; if (size > limit) { reject(new Error('archivo demasiado grande')); req.destroy(); } else chunks.push(c); });
    req.on('end', () => resolve(Buffer.concat(chunks)));
    req.on('error', reject);
  });
}
function json(res, code, obj) {
  res.writeHead(code, { 'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store' });
  res.end(JSON.stringify(obj));
}

let serverPort = 0;
const server = http.createServer(async (req, res) => {
  try {
    // Solo peticiones desde la propia ventana (mismo origen)
    if (req.method === 'POST') {
      const origin = req.headers.origin;
      if (origin && origin !== `http://127.0.0.1:${serverPort}`) return json(res, 403, { error: 'origen no permitido' });
    }
    const url = new URL(req.url, 'http://127.0.0.1');
    const p = url.pathname;

    if (req.method === 'GET' && p === '/') {
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8', 'Cache-Control': 'no-store' });
      return res.end(fs.readFileSync(UI_FILE));
    }
    if (req.method === 'GET' && p === '/api/packs') {
      return json(res, 200, { icons: ICONS, packs: getPacks() });
    }
    if (req.method === 'GET' && p.startsWith('/thumb/')) {
      const [, , kind, d, f] = p.split('/').map(decodeURIComponent);
      const root = kind === 'c' ? resolveIn(PACKS, d) : kind === 'i' ? resolveIn(ICONS, d) : null;
      const folder = root && (kind === 'c' ? path.join(root, 'icons') : root);
      if (!folder || !listImages(folder).includes(f)) { res.writeHead(404); return res.end(); }
      res.writeHead(200, { 'Content-Type': MIME[path.extname(f).toLowerCase()] || 'application/octet-stream',
        'Cache-Control': 'max-age=3600' });
      return fs.createReadStream(path.join(folder, f)).pipe(res);
    }
    if (req.method === 'POST' && p === '/api/install') {
      const { id } = JSON.parse((await readBody(req, 4096)).toString() || '{}');
      installPack(String(id || '')); updateTitles();
      return json(res, 200, { ok: true });
    }
    if (req.method === 'POST' && p === '/api/uninstall') {
      const { id } = JSON.parse((await readBody(req, 4096)).toString() || '{}');
      uninstallPack(String(id || '')); updateTitles();
      return json(res, 200, { ok: true });
    }
    if (req.method === 'POST' && p === '/api/upload') {
      const fname = path.basename(String(url.searchParams.get('name') || 'pack.streamDeckIconPack'));
      if (!PACK_RE.test(fname)) return json(res, 400, { error: 'Elige un archivo .streamDeckIconPack o .zip' });
      const buf = await readBody(req);
      const tmpFile = path.join(os.tmpdir(), `${Date.now()}_${fname}`);
      fs.writeFileSync(tmpFile, buf);
      try {
        const r = await addPack(tmpFile, { keepOriginal: false });
        fs.copyFileSync(tmpFile, path.join(ORIG, fname));
        updateTitles();
        return json(res, 200, { ok: true, ...r });
      } finally { fs.rmSync(tmpFile, { force: true }); }
    }
    if (req.method === 'POST' && p === '/api/open-folder') {
      spawn('explorer.exe', [ICONS], { detached: true, stdio: 'ignore' }).unref();
      return json(res, 200, { ok: true });
    }
    res.writeHead(404); res.end();
  } catch (e) {
    log(`Error en la ventana: ${e.message}`);
    json(res, 500, { error: e.message });
  }
});
server.listen(0, '127.0.0.1', () => {
  serverPort = server.address().port;
  log(`Ventana disponible en http://127.0.0.1:${serverPort}/`);
});

function openWindow() {
  const url = `http://127.0.0.1:${serverPort}/`;
  // Edge en modo app = ventana sin barra de navegador. Si falla, navegador por defecto.
  const opts = { detached: true, stdio: 'ignore', windowsHide: true, windowsVerbatimArguments: true };
  const p = spawn('cmd.exe', ['/c', `start "" msedge --app=${url} --window-size=720,620`], opts);
  p.on('exit', (code) => {
    if (code !== 0) spawn('cmd.exe', ['/c', `start "" ${url}`], opts).unref();
  });
  p.unref();
}

// ---------- Comunicación con OpenDeck ----------
const contexts = new Set();
function send(obj) { if (ws && ws.readyState === 1) ws.send(JSON.stringify(obj)); }
function updateTitles() {
  const all = getPacks();
  const inst = all.filter((x) => x.installed).length;
  for (const ctx of contexts) {
    send({ event: 'setTitle', context: ctx, payload: { title: `Packs\n${inst}/${all.length}`, target: 0 } });
  }
}
function flashOk() { for (const ctx of contexts) send({ event: 'showOk', context: ctx }); }

function connect() {
  if (typeof WebSocket === 'undefined') {
    log('Este Node.js no trae WebSocket nativo (hace falta Node 22 o superior).');
    return;
  }
  ws = new WebSocket(`ws://127.0.0.1:${args.port}`);
  ws.onopen = () => {
    ws.send(JSON.stringify({ event: args.registerEvent, uuid: args.pluginUUID }));
    log('Conectado a OpenDeck');
  };
  ws.onmessage = (ev) => {
    let msg;
    try { msg = JSON.parse(typeof ev.data === 'string' ? ev.data : ev.data.toString()); } catch (_) { return; }
    if (msg.event === 'willAppear') { contexts.add(msg.context); updateTitles(); }
    else if (msg.event === 'willDisappear') contexts.delete(msg.context);
    else if (msg.event === 'keyDown') openWindow();
  };
  ws.onclose = () => process.exit(0);
  ws.onerror = (e) => log(`WebSocket: ${e.message || 'error'}`);
}

// ---------- Arranque ----------
connect();
scanDownloads();
try {
  fs.watch(WATCH, (_e, f) => { if (f && /\.streamdeckiconpack$/i.test(f)) enqueue(path.join(WATCH, f)); });
  log(`Vigilando ${WATCH}`);
} catch (e) { log(`No se pudo vigilar ${WATCH}: ${e.message}`); }
