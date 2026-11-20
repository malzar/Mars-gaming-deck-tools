// ------------------------------------------------------------- paletas --
const BUILTIN = [
  ['Arcoíris', ['#ff0000', '#ff8a00', '#ffe600', '#00d84a', '#00b3ff', '#5a3dff', '#d400ff']],
  ['Pastel', ['#ffb3c7', '#ffd1a8', '#b8f2d0', '#c9b8ff']],
  ['Aurora', ['#00ffa3', '#00d4ff', '#7a5cff', '#d04dff']],
  ['Atardecer', ['#ffb347', '#ff6a3d', '#e8366f', '#7b2d8e']],
  ['Océano', ['#00e5ff', '#0091ea', '#1a3cff', '#002f6c']],
  ['Fuego', ['#ffe600', '#ff8a00', '#ff2a00', '#8a0000']],
  ['Synthwave', ['#ff2fd1', '#a238ff', '#2de2ff']],
  ['Hielo', ['#ffffff', '#bfe9ff', '#5ec8ff', '#2a7fff']],
  ['Bosque', ['#c6ff6b', '#3ddc5a', '#0e8a4a', '#0b4d3a']],
  ['Neón', ['#39ff14', '#ff073a', '#00f0ff', '#fff01f']],
];
// Nombre de una paleta de serie en el idioma del panel (el nombre guardado no cambia)
const palName = (n, mine) => (!mine && t['pal_' + n.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase()]) || n;
let custom = [];
const allPals = () => BUILTIN.concat(custom.map((p) => [p.name, p.colors, true]));
const grad = (c) => c.length > 1 ? `linear-gradient(90deg,${c.join(',')})` : c[0];
const hexRgb = (h) => { const n = parseInt(h.slice(1), 16); return [n >> 16, n >> 8 & 255, n & 255]; };
function samplePal(cols, x) {
  if (cols.length === 1) return hexRgb(cols[0]);
  const f = ((x % 1) + 1) % 1 * cols.length, i = Math.floor(f), k = f - i, a = hexRgb(cols[i]), b = hexRgb(cols[(i + 1) % cols.length]);
  return a.map((v, j) => Math.round(v + (b[j] - v) * k));
}

// Rejilla de paletas reutilizable: sel() devuelve el nombre elegido, pick(nombre, colores)
function palGrid(el, sel, pick) {
  el.innerHTML = allPals().map(([n, c, mine], i) => `<button class="pc ${n === sel() ? 'on' : ''}" data-p="${i}"><div class="sw" style="background:${grad(c)}"></div>${esc(palName(n, mine))}${mine ? `<span class="del" data-d="${i}" title="×">✕</span>` : ''}</button>`).join('')
    + `<button class="pc" data-new="1" style="text-align:center;color:var(--muted)">+ ${t.newPal}</button>`;
  el.querySelectorAll('[data-p]').forEach((b) => b.onclick = (e) => {
    const p = allPals()[+b.dataset.p];
    if (e.target.dataset.d !== undefined) {
      if (!confirm(`${t.delPal} "${p[0]}"?`)) return;
      custom = custom.filter((x) => x.name !== p[0]); api('/api/palettes', custom).catch(() => {}); redrawPals(); return;
    }
    pick(p[0], p[1].slice()); redrawPals();
  });
  el.querySelector('[data-new]').onclick = () => openEditor(el, pick);
}

let edStops = [], edPick = null;
function openEditor(after, pick) {
  edPick = pick; edStops = ['#ff2fd1', '#2de2ff']; $('palName').value = ''; $('palErr').textContent = '';
  after.after($('palEd')); $('palEd').classList.remove('hidden'); renderStops(); $('palName').focus();
}
function renderStops() {
  $('palStops').innerHTML = edStops.map((c, i) => `<span style="display:flex;flex-direction:column;align-items:center;gap:2px"><input type="color" value="${c}" data-s="${i}"><button data-r="${i}" style="padding:0 6px;border:0;font-size:11px" ${edStops.length <= 2 ? 'disabled' : ''}>✕</button></span>`).join('');
  $('palSw').style.background = grad(edStops); $('palAdd').disabled = edStops.length >= 8;
  $('palStops').querySelectorAll('[data-s]').forEach((e) => e.oninput = () => { edStops[+e.dataset.s] = e.value; $('palSw').style.background = grad(edStops); });
  $('palStops').querySelectorAll('[data-r]').forEach((e) => e.onclick = () => { edStops.splice(+e.dataset.r, 1); renderStops(); });
}
$('palAdd').onclick = () => { if (edStops.length < 8) { edStops.push(edStops[edStops.length - 1]); renderStops(); } };
$('palCancel').onclick = () => $('palEd').classList.add('hidden');
$('palName').oninput = () => { $('palErr').textContent = ''; };
$('palSave').onclick = () => {
  const n = $('palName').value.trim();
  if (!n) { $('palErr').textContent = t.needName; return; }
  custom = custom.filter((x) => x.name !== n).concat([{ name: n, colors: edStops.slice() }]);
  api('/api/palettes', custom).catch((e) => msg(e.message, 'err'));
  $('palEd').classList.add('hidden'); edPick(n, edStops.slice()); redrawPals();
};

