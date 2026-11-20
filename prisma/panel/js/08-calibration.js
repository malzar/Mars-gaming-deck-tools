// ----------------------------------------------------------- calibración --
let calZone = 'keys', calOn = false, calBusy = false, calPending = false;
const CAL0 = { dx: 0, dy: 0, size: 100 };
const ZLBL = { keys: 'calKeys', side: 'calSide', strip: 'calStrip' };
const calZones = () => current.cal_zones || [];
const calSize = () => (calZones().find(([z]) => z === calZone) || ['keys', [85, 85]])[1];
const calAdj = () => {
  const c = draft.calibration = draft.calibration || {};
  for (const [z] of calZones()) c[z] = c[z] || { ...CAL0 };
  return c[calZone] || (c[calZone] = { ...CAL0 });
};
function calPattern(w, h) {
  const c = document.createElement('canvas'); c.width = w; c.height = h; const x = c.getContext('2d'), u = Math.min(w, h) / 85, r = Math.max(1, Math.round(3 * u));
  ['#ff2020', '#ff8a00', '#ffe600', '#20d040', '#00c8ff', '#3050ff', '#a040ff'].forEach((col, i) => { x.fillStyle = col; x.fillRect(i * r, i * r, w - i * 2 * r, h - i * 2 * r); });
  x.fillStyle = '#000'; x.fillRect(7 * r, 7 * r, w - 14 * r, h - 14 * r); x.fillStyle = '#0c1440'; x.fillRect(8 * r, 8 * r, w - 16 * r, h - 16 * r);
  const cx = w / 2, top = h / 2 - 12 * u;
  x.fillStyle = '#fff'; x.beginPath(); x.moveTo(cx, top); x.lineTo(cx + 8 * u, top + 16 * u); x.lineTo(cx - 8 * u, top + 16 * u); x.fill();
  x.fillRect(cx - 6 * u, h / 2 + 6 * u, 12 * u, 4 * u);
  for (const [px, py] of [[0, 0], [w - 5 * u, 0], [0, h - 5 * u], [w - 5 * u, h - 5 * u]]) x.fillRect(px, py, 5 * u, 5 * u);
  return c;
}
function drawCal() {
  const [w, h] = calSize(), a = calAdj(), cv = $('calCv'), k = 120 / Math.max(w, h);
  cv.width = Math.round(w * k); cv.height = Math.round(h * k); cv.style.width = cv.width + 'px'; cv.style.height = cv.height + 'px';
  const x = cv.getContext('2d'), s = a.size / 100;
  x.fillStyle = '#000'; x.fillRect(0, 0, cv.width, cv.height);
  x.drawImage(calPattern(w, h), ((w - w * s) / 2 + a.dx) * k, ((h - h * s) / 2 + a.dy) * k, w * s * k, h * s * k);
}
function fillCal() {
  const zs = calZones();
  $('calCard').classList.toggle('hidden', !zs.length);
  if (!zs.length) return;
  if (!zs.some(([z]) => z === calZone)) calZone = zs[0][0];
  $('calZone').innerHTML = zs.map(([z]) => `<button data-z="${z}" class="${z === calZone ? 'on' : ''}">${t[ZLBL[z]] || z}</button>`).join('');
  $('calZone').querySelectorAll('button').forEach((b) => b.onclick = () => { calZone = b.dataset.z; fillCal(); sendCal(calZone); });
  const a = calAdj();
  $('calX').value = a.dx; $('calY').value = a.dy; $('calS').value = a.size;
  $('calXV').textContent = a.dx + ' px'; $('calYV').textContent = a.dy + ' px'; $('calSV').textContent = a.size + ' %';
  $('calState').textContent = calOn ? t.calOn : ''; $('calStop').disabled = !calOn;
  drawCal();
}
// Manda al deck solo el último cambio, y no hasta que termine el anterior
async function sendCal(zone) {
  if (!calOn) return;
  if (calBusy) { calPending = true; return; }
  calBusy = true;
  try { const r = await api('/api/calibration/test', { id: current.id, calibration: draft.calibration, zone }); if (r.error) throw new Error(r.error); }
  catch (e) { msg(e.message, 'err'); }
  calBusy = false;
  if (calPending) { calPending = false; sendCal(calZone); }
}
async function stopCal() {
  if (!calOn) return;
  calOn = false; fillCal();
  try { await api('/api/calibration/stop', { id: current.id }); } catch (_) {}
}
[['calX', 'dx'], ['calY', 'dy'], ['calS', 'size']].forEach(([el, key]) => $(el).oninput = (e) => { calAdj()[key] = +e.target.value; fillCal(); sendCal(calZone); });
// Botones − / +: un paso por clic; si se mantiene pulsado, repite
document.querySelectorAll('[data-step]').forEach((b) => {
  let timer = null;
  const step = () => {
    const el = $(b.dataset.step), v = Math.min(+el.max, Math.max(+el.min, +el.value + +b.dataset.d));
    if (v === +el.value) return;
    el.value = v; el.dispatchEvent(new Event('input'));
  };
  const stop = () => { clearTimeout(timer); clearInterval(timer); timer = null; };
  b.addEventListener('pointerdown', (e) => { e.preventDefault(); step(); timer = setTimeout(() => { timer = setInterval(step, 120); }, 400); });
  ['pointerup', 'pointerleave', 'pointercancel'].forEach((ev) => b.addEventListener(ev, stop));
  b.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); step(); } });
});
$('calReset').onclick = () => { draft.calibration[calZone] = { ...CAL0 }; calAdj(); fillCal(); sendCal(calZone); };
$('calTest').onclick = () => { calAdj(); calOn = true; fillCal(); sendCal(calZone); };
$('calStop').onclick = stopCal;
window.addEventListener('pagehide', () => { if (calOn) navigator.sendBeacon('/api/calibration/stop', new Blob([JSON.stringify({ id: current.id })], { type: 'application/json' })); });


$('identify').onclick = () => api('/api/identify', { id: current.id }).catch((e) => msg(e.message, 'err'));
