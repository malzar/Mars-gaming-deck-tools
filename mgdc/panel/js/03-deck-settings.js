// ----------------------------------------------------------------- campos --
function fill() {
  const s = draft, sc = s.schedule;
  $('brightness').value = s.brightness; $('brightnessV').textContent = s.brightness + ' %';
  $('neverSleep').checked = s.never_sleep; $('sleepMinutes').value = s.sleep_minutes; $('swallow').checked = s.swallow_wake_press;
  $('schedEnabled').checked = sc.enabled;
  $('city').value = sc.city; $('lat').value = sc.lat; $('lon').value = sc.lon; $('offset').value = sc.offset_minutes;
  $('dayStart').value = sc.day_start; $('nightStart').value = sc.night_start;
  $('dayBri').value = sc.day_brightness; $('nightBri').value = sc.night_brightness; $('transition').value = sc.transition_minutes;
  $('info').innerHTML = infoRows()
  fillAnims();
  update();
}

function infoList() {
  const c = current, sc = c.screen || [];
  return [[t.model, c.name], ['VID:PID', c.vid_pid || '—'], [t.serial, c.serial || '—'], [t.id, c.id],
    [t.protocol, 'v' + c.protocol], [t.screenI, sc.length ? `${sc[0]} × ${sc[1]} · ${t.keys} ${c.key_size} × ${c.key_size}` : '—'],
    [t.controls, c.layout]];
}
function infoRows() {
  return infoList().map(([a, b]) => `<span>${esc(a)}</span><span style="color:var(--text)">${esc(b)}</span>`).join('');
}

function update() {
  const s = draft, sc = s.schedule;
  $('brightnessV').textContent = s.brightness + ' %';
  $('sleepMinutes').disabled = s.never_sleep; $('sleepLbl').className = s.never_sleep ? 'off' : '';
  $('schedBody').classList.toggle('hidden', !sc.enabled); $('schedOff').classList.toggle('hidden', sc.enabled);
  $('briNote').classList.toggle('hidden', !sc.enabled);
  document.querySelectorAll('[data-mode]').forEach((b) => b.classList.toggle('on', b.dataset.mode === sc.mode));
  $('sunBox').classList.toggle('hidden', sc.mode !== 'sun'); $('fixBox').classList.toggle('hidden', sc.mode !== 'fixed');
  $('dayBriV').textContent = sc.day_brightness + ' %'; $('nightBriV').textContent = sc.night_brightness + ' %';
  drawBar();
}

function windowMinutes() {
  const sc = draft.schedule;
  if (sc.mode === 'sun' && sun && !sun.error) {
    const r = toMin(sun.sunrise) + (+sc.offset_minutes || 0), s = toMin(sun.sunset) + (+sc.offset_minutes || 0);
    if (r < s) return [r, s];
  }
  return [toMin(sc.day_start) ?? 450, toMin(sc.night_start) ?? 1260];
}

// Mismo cálculo que el plugin (schedule.rs)
function levelAt(m) {
  const sc = draft.schedule, [day, night] = windowMinutes(), tr = +sc.transition_minutes || 0;
  const d = +sc.day_brightness, n = +sc.night_brightness, lerp = (a, b, x) => Math.round(a + (b - a) * Math.min(1, Math.max(0, x)));
  if (tr > 0 && m >= day && m < day + tr) return [lerp(n, d, (m - day) / tr), true];
  if (m >= day && m < night) return [d, true];
  if (tr > 0 && m >= night && m < night + tr) return [lerp(d, n, (m - night) / tr), false];
  return [n, false];
}

function drawBar() {
  let h = '';
  for (let m = 0; m < 1440; m += 15) {
    const [v, isDay] = levelAt(m);
    h += `<div style="position:absolute;left:${m / 14.4}%;width:1.3%;bottom:0;height:${Math.max(6, v)}%;background:${isDay ? 'var(--day)' : 'var(--night)'}"></div>`;
  }
  const now = new Date(), nm = now.getHours() * 60 + now.getMinutes();
  h += `<div style="position:absolute;left:${nm / 14.4}%;top:0;bottom:0;width:2px;background:var(--text);opacity:.6"></div>`;
  $('bar').innerHTML = h;
}

async function refreshSun() {
  const sc = draft.schedule;
  try {
    sun = await api(`/api/sun?lat=${encodeURIComponent(sc.lat)}&lon=${encodeURIComponent(sc.lon)}`);
    $('sunrise').textContent = sun.error ? '—' : sun.sunrise; $('sunset').textContent = sun.error ? '—' : sun.sunset;
    if (sun.error) msg(t.polar, 'err');
  } catch (_) { sun = null; }
  drawBar();
}

// --------------------------------------------------------------- eventos --
$('brightness').oninput = (e) => {
  draft.brightness = +e.target.value; update();
  clearTimeout(briTimer);
  briTimer = setTimeout(() => api('/api/brightness', { id: current.id, value: draft.brightness }).catch(() => {}), 120);
};
$('neverSleep').onchange = (e) => { draft.never_sleep = e.target.checked; update(); };
$('sleepMinutes').onchange = (e) => { draft.sleep_minutes = Math.max(1, +e.target.value || 15); };
$('swallow').onchange = (e) => { draft.swallow_wake_press = e.target.checked; };
$('schedEnabled').onchange = (e) => { draft.schedule.enabled = e.target.checked; update(); };
document.querySelectorAll('[data-mode]').forEach((b) => b.onclick = () => { draft.schedule.mode = b.dataset.mode; update(); });
// Buscar la localidad (cualquier pueblo o ciudad): Prisma pregunta a Open-Meteo
// y las respuestas salen en la lista del campo. Sin internet quedan las de serie.
let places = [], placeTimer = null;
const placeLabel = (p) => p.region ? `${p.name}, ${p.region}` : p.name;
const presets = () => CITIES.map(([n]) => `<option value="${esc(n)}">`).join('');
$('city').oninput = () => {
  clearTimeout(placeTimer);
  const q = $('city').value.trim();
  if (q.length < 2 || places.some((p) => placeLabel(p) === q)) return;
  placeTimer = setTimeout(async () => {
    try {
      const r = await (await fetch('/api/places?q=' + encodeURIComponent(q))).json();
      if ($('city').value.trim() !== q) return;
      places = r.places || [];
      $('cities').innerHTML = places.length ? places.map((p) => `<option value="${esc(placeLabel(p))}">`).join('') : presets();
      if (!places.length) msg(r.error || t.noPlace, 'err');
    } catch (e) { /* sin conexión: quedan las de serie */ }
  }, 350);
};
$('city').onchange = (e) => {
  const v = e.target.value.trim();
  const p = places.find((x) => placeLabel(x) === v) || places.find((x) => x.name.toLowerCase() === v.toLowerCase());
  const c = CITIES.find(([n]) => n.toLowerCase() === v.toLowerCase());
  const pick = p ? [p.name, p.lat, p.lon] : c;
  if (!pick) { msg(t.cityHint, 'err'); return; }
  draft.schedule.city = pick[0]; draft.schedule.lat = pick[1]; draft.schedule.lon = pick[2];
  $('lat').value = pick[1]; $('lon').value = pick[2]; refreshSun();
};
$('lat').onchange = (e) => { draft.schedule.lat = +e.target.value; refreshSun(); };
$('lon').onchange = (e) => { draft.schedule.lon = +e.target.value; refreshSun(); };
$('offset').onchange = (e) => { draft.schedule.offset_minutes = Math.round(+e.target.value || 0); drawBar(); };
$('dayStart').onchange = (e) => { draft.schedule.day_start = e.target.value; drawBar(); };
$('nightStart').onchange = (e) => { draft.schedule.night_start = e.target.value; drawBar(); };
$('dayBri').oninput = (e) => { draft.schedule.day_brightness = +e.target.value; update(); };
$('nightBri').oninput = (e) => { draft.schedule.night_brightness = +e.target.value; update(); };
$('transition').onchange = (e) => { draft.schedule.transition_minutes = Math.max(0, Math.round(+e.target.value || 0)); drawBar(); };

