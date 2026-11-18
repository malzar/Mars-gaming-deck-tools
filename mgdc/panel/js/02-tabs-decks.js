// --------------------------------------------------------------- pestañas --
let tab = 'screen';
try { tab = localStorage.getItem('prisma-tab') || 'screen'; } catch (_) {}
function showTab(name) {
  tab = name;
  try { localStorage.setItem('prisma-tab', name); } catch (_) {}
  document.querySelectorAll('#tabs [data-go]').forEach((b) => b.classList.toggle('on', b.dataset.go === name));
  document.querySelectorAll('[data-tab]').forEach((e) => e.classList.toggle('tabhide', e.dataset.tab !== name));
  // Al salir de "Iconos" se quita la imagen de prueba
  if (name !== 'icons' && typeof stopCal === 'function') stopCal();
}
document.querySelectorAll('#tabs [data-go]').forEach((b) => b.onclick = () => showTab(b.dataset.go));

// ------------------------------------------------------------------ decks --
function renderDevices() {
  $('devices').innerHTML = devices.map((d) => `<button class="dev ${current && d.id === current.id ? 'on' : ''}" data-id="${esc(d.id)}">
    <strong style="font-size:13px">${esc(d.name)}</strong><small>● ${t.connected}</small></button>`).join('');
  $('devices').querySelectorAll('[data-id]').forEach((b) => b.onclick = () => {
    if (dirty() && !confirm(t.unsaved)) return;
    stopCal(); select(b.dataset.id);
  });
}

function select(id) {
  current = devices.find((d) => d.id === id) || devices[0];
  draft = JSON.parse(JSON.stringify(current.settings));
  // Si el deck no admite la animación guardada, se muestra "Ninguna"
  const kinds = current.anim_kinds || [];
  for (const w of ['startup', 'shutdown']) if (kinds.length && !kinds.includes(draft.animations[w].kind)) draft.animations[w].kind = 'none';
  saved = JSON.stringify(draft);
  calZone = 'keys'; renderDevices(); fill(); fillCal(); refreshSun(); loadBackground(); prf = null; if (tab === 'links') loadProfiles();
}

