const t = JSON.parse(document.getElementById('i18n').textContent);

// Pregunta con un aviso del propio panel. Siempre lleva «Cancelar» (también
// con Esc o pulsando fuera, y devuelve null); `buttons` son los demás:
// [{ label, value, danger }]. Sin `buttons`, un «Aceptar» que devuelve true.
function ask(text, buttons) {
  buttons = buttons || [{ label: t.ok, value: true, danger: true }];
  return new Promise((done) => {
    const box = document.createElement('div');
    box.className = 'ask';
    box.innerHTML = `<div class="ask-card" role="alertdialog" aria-modal="true"><p></p><div class="row"><button data-a="-1">${t.cancel}</button>${buttons
      .map((b, i) => `<button data-a="${i}" class="${b.danger ? 'primary danger' : ''}">${esc(b.label)}</button>`).join('')}</div></div>`;
    box.querySelector('p').textContent = text;
    const close = (v) => { document.removeEventListener('keydown', key, true); box.remove(); done(v); };
    const key = (e) => { if (e.key === 'Escape') { e.preventDefault(); close(null); } };
    box.onclick = (e) => {
      if (e.target === box) return close(null);
      const b = e.target.closest('button');
      if (b) close(+b.dataset.a < 0 ? null : buttons[+b.dataset.a].value);
    };
    document.addEventListener('keydown', key, true);
    document.body.appendChild(box);
    box.querySelector('[data-a="-1"]').focus();
  });
}

const CITIES = [
  ['Madrid', 40.4168, -3.7038], ['Barcelona', 41.3874, 2.1686], ['Valencia', 39.4699, -0.3763], ['Sevilla', 37.3891, -5.9845],
  ['Zaragoza', 41.6488, -0.8891], ['Málaga', 36.7213, -4.4214], ['Murcia', 37.9922, -1.1307], ['Palma', 39.5696, 2.6502],
  ['Las Palmas de Gran Canaria', 28.1235, -15.4363], ['Santa Cruz de Tenerife', 28.4636, -16.2518], ['Bilbao', 43.2630, -2.9350],
  ['Alicante', 38.3452, -0.4810], ['Córdoba', 37.8882, -4.7794], ['Valladolid', 41.6523, -4.7245], ['Vigo', 42.2406, -8.7207],
  ['Gijón', 43.5322, -5.6611], ['A Coruña', 43.3623, -8.4115], ['Granada', 37.1773, -3.5986], ['Oviedo', 43.3619, -5.8494],
  ['Pamplona', 42.8125, -1.6458], ['Santander', 43.4623, -3.8100], ['San Sebastián', 43.3183, -1.9812], ['Salamanca', 40.9701, -5.6635],
  ['Toledo', 39.8628, -4.0273], ['Cádiz', 36.5271, -6.2886], ['Almería', 36.8340, -2.4637], ['Badajoz', 38.8794, -6.9707],
  ['Logroño', 42.4627, -2.4450], ['Lisboa', 38.7223, -9.1393], ['Londres', 51.5072, -0.1276], ['París', 48.8566, 2.3522],
  ['Berlín', 52.5200, 13.4050], ['Roma', 41.9028, 12.4964], ['Nueva York', 40.7128, -74.0060], ['Ciudad de México', 19.4326, -99.1332],
  ['Buenos Aires', -34.6037, -58.3816], ['Bogotá', 4.7110, -74.0721], ['Santiago de Chile', -33.4489, -70.6693], ['Lima', -12.0464, -77.0428],
  ['Tokio', 35.6762, 139.6503],
];

const $ = (id) => document.getElementById(id);
let devices = [], current = null, draft = null, saved = '';
let sun = null, briTimer = null;

document.querySelectorAll('[data-t]').forEach((e) => { e.textContent = t[e.dataset.t]; });
document.querySelectorAll('[data-tp]').forEach((e) => { e.placeholder = t[e.dataset.tp]; });
$('cities').innerHTML = CITIES.map(([n]) => `<option value="${n}">`).join('');
$('city').setAttribute('autocomplete', 'off');

const api = (path, body) => fetch(path, body
  ? { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }
  : { cache: 'no-store' }).then((r) => { if (!r.ok) throw new Error('HTTP ' + r.status); return r.json(); });
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const toMin = (s) => { const m = /^(\d{1,2}):(\d{2})$/.exec(s || ''); return m ? (+m[1]) * 60 + (+m[2]) : null; };
const dirty = () => draft && JSON.stringify(draft) !== saved;

function msg(text, kind) { $('msg').textContent = text || ''; $('msg').className = 'msg ' + (kind || ''); if (text && kind !== 'wait') setTimeout(() => { if ($('msg').textContent === text) $('msg').textContent = ''; }, 3000); }

