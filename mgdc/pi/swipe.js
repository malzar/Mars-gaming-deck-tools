// Bloque «Pantalla táctil» de las ventanas de ajustes de las ruedas: qué hace
// deslizar por la pantalla tras tocar la zona de la rueda (ver gestures.rs).
// Cada ventana lo monta al conectarse y lo refresca cuando le llegan ajustes.
// Guarda en los ajustes de la acción: swipe ("off" | "same" | "other"),
// swipe_action ("track" | "volume") y swipe_invert.
window.PrismaSwipe = (function () {
  const ACTIONS = ['track', 'volume'];
  let get = null, save = null, box = null;

  const row = (label, control) => `<div class="row"><label>${label}</label>${control}</div>`;
  const opt = (v, key) => `<option value="${v}">${T(key)}</option>`;
  const $ = (id) => document.getElementById(id);

  function show() {
    if (!box) return;
    const s = get(), mode = s.swipe || 'off';
    $('swipe').value = mode;
    $('swipe_action').value = ACTIONS.includes(s.swipe_action) ? s.swipe_action : 'track';
    $('swipe_invert').checked = !!s.swipe_invert;
    $('swipeActRow').style.display = mode === 'other' ? '' : 'none';
    $('swipeInvRow').style.display = mode === 'off' ? 'none' : '';
    $('swipeNote').textContent = T(mode === 'off' ? 'sw_note_off' : 'sw_note_on');
  }

  function set(key, value) { get()[key] = value; save(); show(); }

  // `ai`: el actionInfo de OpenDeck (solo se monta en ruedas)
  function mount(ai, getSettings, saveSettings) {
    if (box || (ai.payload && ai.payload.controller) !== 'Encoder') return;
    get = getSettings; save = saveSettings;
    box = document.createElement('div');
    box.innerHTML =
      `<p class="note" style="border-top:1px solid var(--border);padding-top:8px;margin-top:12px">${T('sw_title')}</p>` +
      row(T('sw_mode'), `<select id="swipe">${opt('off', 'sw_off')}${opt('same', 'sw_same')}${opt('other', 'sw_other')}</select>`) +
      `<div id="swipeActRow">${row(T('sw_action'), `<select id="swipe_action">${opt('track', 'sw_track')}${opt('volume', 'sw_volume')}</select>`)}</div>` +
      `<div id="swipeInvRow">${row('', `<label><input type="checkbox" id="swipe_invert"> ${T('sw_invert')}</label>`)}</div>` +
      `<p class="note" id="swipeNote"></p>`;
    document.body.appendChild(box);
    $('swipe').onchange = (e) => set('swipe', e.target.value);
    $('swipe_action').onchange = (e) => set('swipe_action', e.target.value);
    $('swipe_invert').onchange = (e) => set('swipe_invert', e.target.checked);
    show();
  }

  return { mount, refresh: show };
})();
