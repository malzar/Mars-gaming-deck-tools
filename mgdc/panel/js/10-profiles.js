// ------------------------------------------------- iconos de los perfiles --
let prf = null;
const hasWheel = () => current && (current.cal_zones || []).some((z) => z[0] === 'strip');
async function loadProfiles() {
  if (!current) return;
  $('prfBorderRow').classList.toggle('hidden', !hasWheel()); $('prfNoWheel').classList.toggle('hidden', hasWheel());
  try { prf = await api('/api/profiles?id=' + encodeURIComponent(current.id)); renderProfiles(); } catch (e) { $('prfList').textContent = e.message; }
}
const iconUrl = (p) => `/api/profiles/icon?id=${encodeURIComponent(current.id)}&profile=${encodeURIComponent(p.name)}&v=${encodeURIComponent(p.icon || '')}`;
// Perfil que se está editando: { name, newName, icon } (icon: undefined = sin
// cambios, null = quitarlo, 'data:…' = el nuevo). Se aplica todo al guardar.
let prfEdit = null;
function iconCell(p, col, src) {
  const outline = p.active ? `outline:3px solid ${col};outline-offset:1px` : '';
  return src ? `<img class="ic" src="${src}" style="display:inline-block;vertical-align:middle;${outline}">` : `<div class="noi" style="display:inline-flex;vertical-align:middle">${t.prfNoIcon}</div>`;
}
function renderProfiles() {
  if (!prf) return;
  $('prfBorder').value = prf.border || '#2ecc71';
  const col = $('prfBorder').value;
  if (prfEdit && !(prf.profiles || []).some((p) => p.name === prfEdit.name)) prfEdit = null;
  $('prfList').innerHTML = (prf.profiles || []).map((p, i) => {
    const indent = p.depth ? `<span style="display:inline-block;width:${(p.depth - 1) * 26 + 6}px"></span><span class="tw">└</span>` : '';
    const first = `<td style="white-space:nowrap;width:${44 + (p.depth || 0) * 26}px">${indent}`;
    if (prfEdit && prfEdit.name === p.name) {
      const src = prfEdit.icon === undefined ? (p.icon ? iconUrl(p) : null) : prfEdit.icon;
      const isDefault = p.name === 'Default';
      return `<tr class="editing${p.depth ? ' sub' : ''}">${first}${iconCell(p, col, src)}</td><td colspan="3"><div class="prf-edit">
        <label for="prfEditName">${t.prfName}</label>
        <input type="text" id="prfEditName" maxlength="40" value="${esc(prfEdit.newName)}"${isDefault ? ' disabled' : ''}>
        <div class="row"><button id="prfEditPick">${src ? t.prfChange : t.prfPut}</button>${src ? `<button id="prfEditRm">✕ ${t.prfRemove}</button>` : ''}
          <span style="flex:1"></span><button id="prfEditCancel">${t.cancel}</button><button id="prfEditSave" class="primary">${t.save}</button></div>
        <p>${isDefault ? t.prfDefaultNote : t.prfEditNote}</p></div></td></tr>`;
    }
    const canDelete = !p.active && p.name !== 'Default';
    return `<tr class="${p.depth ? 'sub' : ''}">${first}${iconCell(p, col, p.icon ? iconUrl(p) : null)}</td>
    <td>${esc(p.nombre || p.name)}${p.active ? `<span class="act">${t.prfActive}</span>` : ''}${p.kids ? ` <span class="kids">▾ ${p.kids} ${p.kids === 1 ? t.prfSub1 : t.prfSubN}</span>` : ''}</td>
    <td class="btns"><button class="ico" data-edit="${i}" title="${t.prfEditTip}" aria-label="${t.prfEditTip}">✎</button> <button data-sub="${i}" title="${t.prfSubTip}">${t.prfSub}</button>${canDelete ? ` <button class="ico del" data-del="${i}" title="${t.prfDeleteTip}" aria-label="${t.prfDeleteTip}">✕</button>` : ''}</td>
    <td class="grip" data-grip="${i}" title="${t.prfDrag}">⠿</td></tr>`;
  }).join('') || `<tr><td style="color:var(--muted)">${t.prfNone}</td></tr>`;
  renderNew();
  $('prfList').querySelectorAll('[data-grip]').forEach((g) => g.onpointerdown = (e) => { if (!prfEdit) dragProfile(e, +g.dataset.grip); });
  $('prfList').querySelectorAll('[data-sub]').forEach((b) => b.onclick = () => { prfForm = true; prfParent = prf.profiles[+b.dataset.sub].name; renderNew(); $('prfName').focus(); });
  $('prfList').querySelectorAll('[data-del]').forEach((b) => b.onclick = () => deleteProfile(prf.profiles[+b.dataset.del]));
  $('prfList').querySelectorAll('[data-edit]').forEach((b) => b.onclick = () => {
    const p = prf.profiles[+b.dataset.edit];
    prfEdit = { name: p.name, newName: p.name, icon: undefined };
    renderProfiles();
  });
  if (prfEdit) {
    const name = $('prfEditName');
    name.oninput = () => { prfEdit.newName = name.value; };
    name.onkeydown = (e) => { if (e.key === 'Enter') saveEdit(); if (e.key === 'Escape') cancelEdit(); };
    $('prfEditPick').onclick = () => $('prfFile').click();
    if ($('prfEditRm')) $('prfEditRm').onclick = () => { prfEdit.icon = null; renderProfiles(); };
    $('prfEditCancel').onclick = cancelEdit;
    $('prfEditSave').onclick = saveEdit;
    if (!name.disabled) { name.focus(); name.setSelectionRange(name.value.length, name.value.length); }
  }
}
function cancelEdit() { prfEdit = null; renderProfiles(); }
// Guarda lo editado: primero el nombre (el deck se reconecta en OpenDeck) y
// luego el icono, ya con el nombre nuevo
async function saveEdit() {
  const e = prfEdit; if (!e) return;
  const newName = e.newName.trim();
  const rename = e.name !== 'Default' && newName !== '' && newName !== e.name;
  if (!rename && e.icon === undefined) { cancelEdit(); msg(t.prfNothing, 'ok'); return; }
  document.querySelectorAll('#prfList button, #prfList input').forEach((b) => { b.disabled = true; });
  try {
    let name = e.name;
    if (rename) {
      msg(t.prfRenaming, 'wait');
      const r = await api('/api/profiles/rename', { id: current.id, name: e.name, new_name: newName });
      if (r.error) throw new Error(r.error);
      name = r.name; e.name = name; e.newName = name;
    }
    if (e.icon !== undefined) {
      const r = await api('/api/profiles/icon', { id: current.id, profile: name, image: e.icon });
      if (r.error) throw new Error(r.error);
    }
    prfEdit = null; await loadProfiles(); if (rename) loadLinks(); msg(t.prfRenamed, 'ok');
  } catch (err) {
    // Se queda abierto para corregirlo
    msg(err.message, 'err'); await loadProfiles();
  }
}
// Borrar un perfil; el activo y «Default» no. Si tiene subperfiles, se elige
// entre subirlos un nivel o borrarlo todo
async function deleteProfile(p) {
  const name = p.nombre || p.name;
  const all = p.kids
    ? await ask(t.prfDeleteAskKids.replace('{name}', name).replace('{kids}', `${p.kids} ${p.kids === 1 ? t.prfSub1 : t.prfSubN}`),
      [{ label: t.prfDeleteUp, value: 'up' }, { label: t.prfDeleteAll, value: 'all', danger: true }])
    : await ask(t.prfDeleteAsk.replace('{name}', name), [{ label: t.prfDelete, value: 'one', danger: true }]);
  if (!all) return;
  document.querySelectorAll('#prfList button').forEach((b) => { b.disabled = true; });
  msg(t.prfDeleting, 'wait');
  try {
    const r = await api('/api/profiles/delete', { id: current.id, name: p.name, all: all === 'all' });
    if (r.error) throw new Error(r.error);
    await loadProfiles(); loadLinks(); msg(t.prfDeleted, 'ok');
  } catch (e) { msg(e.message, 'err'); await loadProfiles(); }
}
// Arrastrar por el asa. Sobre el borde de arriba o de abajo de otro perfil:
// se pone encima o debajo, en su mismo grupo. Sobre el centro: se mete dentro,
// como subperfil. Así un subperfil puede pasar a perfil y al revés. Un perfil
// se mueve siempre con sus subperfiles.
function dragProfile(e, from) {
  e.preventDefault();
  const rows = [...$('prfList').querySelectorAll('tr')];
  const list = prf.profiles, me = list[from];
  // Él y sus subperfiles: ahí no se puede soltar
  const inside = new Set([from]);
  for (let i = from + 1; i < list.length && (list[i].depth || 0) > (me.depth || 0); i++) inside.add(i);
  inside.forEach((i) => rows[i].classList.add('dragging'));
  let drop = null;
  const clear = () => rows.forEach((r) => r.classList.remove('over-top', 'over-bottom', 'over-in'));
  const move = (ev) => {
    clear(); drop = null;
    const i = rows.findIndex((r) => { const b = r.getBoundingClientRect(); return ev.clientY >= b.top && ev.clientY < b.bottom; });
    if (i < 0 || inside.has(i)) return;
    const b = rows[i].getBoundingClientRect(), rel = (ev.clientY - b.top) / b.height;
    const where = rel < 0.3 ? 'before' : rel > 0.7 ? 'after' : 'in';
    drop = { i, where };
    rows[i].classList.add(where === 'before' ? 'over-top' : where === 'after' ? 'over-bottom' : 'over-in');
  };
  const up = async () => {
    document.removeEventListener('pointermove', move); document.removeEventListener('pointerup', up);
    clear(); inside.forEach((i) => rows[i].classList.remove('dragging'));
    if (!drop) return;
    const target = list[drop.i];
    let padre, order;
    if (drop.where === 'in') {
      // Dentro del perfil, al final de sus subperfiles
      padre = target.name;
      order = list.filter((p) => p.padre === padre && p.name !== me.name).map((p) => p.name).concat(me.name);
    } else {
      // Encima o debajo, en el grupo del perfil de destino
      padre = target.padre || null;
      order = list.filter((p) => (p.padre || null) === padre && p.name !== me.name).map((p) => p.name);
      const k = order.indexOf(target.name) + (drop.where === 'after' ? 1 : 0);
      order.splice(k, 0, me.name);
    }
    if ((me.padre || null) === padre && JSON.stringify(order) === JSON.stringify(list.filter((p) => (p.padre || null) === padre).map((p) => p.name))) return;
    try {
      const r = await api('/api/profiles/move', { id: current.id, profile: me.name, padre, order });
      if (r.error) throw new Error(r.error);
      await loadProfiles(); loadLinks(); msg(drop.where === 'in' ? t.prfMovedIn + ' «' + (target.nombre || target.name) + '»' : t.prfOrderSaved, 'ok');
    } catch (err) { msg(err.message, 'err'); loadProfiles(); }
  };
  document.addEventListener('pointermove', move); document.addEventListener('pointerup', up);
}

// Perfil nuevo: botón y, al pulsarlo, el formulario
let prfForm = false, prfParent = null;
function renderNew() {
  const box = $('prfNewBox');
  if (!prfForm) {
    box.innerHTML = `<button class="lnkAdd" id="prfNew">${t.prfNew}</button>`;
    $('prfNew').onclick = () => { prfForm = true; prfParent = null; renderNew(); $('prfName').focus(); };
    return;
  }
  const parents = [`<option value="">${t.prfRoot}</option>`].concat((prf ? prf.profiles : []).map((p) => `<option value="${esc(p.name)}" ${p.name === prfParent ? 'selected' : ''}>${'\u00a0\u00a0'.repeat(p.depth || 0)}${esc(p.nombre || p.name)}</option>`)).join('');
  const opts = [`<option value="">${t.prfEmpty}</option>`].concat((prf ? prf.profiles : []).map((p) => `<option value="${esc(p.name)}">${t.prfCopy} ${esc(p.name)}</option>`)).join('');
  box.innerHTML = `<div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap">
      <input type="text" id="prfName" maxlength="40" placeholder="${t.prfName}" style="flex:1;min-width:160px">
      <span style="font-size:12px;color:var(--muted)">${t.prfInside}</span><select id="prfParent" style="width:auto">${parents}</select>
      <span style="font-size:12px;color:var(--muted)">${t.prfStart}</span><select id="prfCopy" style="width:auto">${opts}</select>
      <button class="primary" id="prfCreate">${t.prfCreate}</button><button id="prfCancel">${t.cancel}</button></div>
    <p style="font-size:12px;color:var(--muted);margin:6px 0 0">${t.prfNewNote}</p>`;
  $('prfCancel').onclick = () => { prfForm = false; prfParent = null; renderNew(); };
  $('prfParent').onchange = () => { prfParent = $('prfParent').value || null; };
  $('prfName').onkeydown = (e) => { if (e.key === 'Enter') $('prfCreate').click(); if (e.key === 'Escape') $('prfCancel').click(); };
  $('prfCreate').onclick = async () => {
    const name = $('prfName').value.trim(); if (!name) { $('prfName').focus(); return; }
    const b = $('prfCreate'); b.disabled = true; msg(t.prfCreating, 'wait');
    try {
      const r = await api('/api/profiles/create', { id: current.id, name, copy_of: $('prfCopy').value || null, padre: $('prfParent').value || null });
      if (r.error) throw new Error(r.error);
      prfForm = false; prfParent = null; await loadProfiles(); loadLinks(); msg(t.prfCreated + ': ' + r.name, 'ok');
    } catch (e) { msg(e.message, 'err'); b.disabled = false; }
  };
}
// El icono elegido se ve al momento y se guarda con «Guardar». Un GIF va tal
// cual (para que se mueva); lo demás (también SVG) se pasa a PNG aquí
$('prfFile').onchange = (e) => {
  const f = e.target.files[0]; e.target.value = ''; if (!f || !prfEdit) return;
  const r = new FileReader();
  r.onload = async () => {
    const keep = (url) => { if (prfEdit) { prfEdit.icon = url; renderProfiles(); } };
    if (f.type === 'image/gif') return keep(r.result);
    try {
      const img = await loadImage(r.result), c = document.createElement('canvas'), n = 256;
      c.width = c.height = n;
      const k = Math.max(n / (img.naturalWidth || n), n / (img.naturalHeight || n)), w = (img.naturalWidth || n) * k, h = (img.naturalHeight || n) * k;
      c.getContext('2d').drawImage(img, (n - w) / 2, (n - h) / 2, w, h);
      keep(c.toDataURL('image/png'));
    } catch (_) { msg(t.prfImgInvalid, 'err'); }
  };
  r.readAsDataURL(f);
};
$('prfBorder').onchange = async () => {
  try {
    const r = await api('/api/profiles/border', { id: current.id, border: $('prfBorder').value });
    if (r.error) throw new Error(r.error);
    if (prf) prf.border = $('prfBorder').value;
    renderProfiles(); msg(t.prfBorderSaved, 'ok');
  } catch (e) { msg(e.message, 'err'); }
};
document.querySelector('#tabs [data-go="links"]').addEventListener('click', loadProfiles);

$('save').onclick = async () => {
  try {
    if (lnkDirty()) {
      const r = await api('/api/links', lnk);
      lnk = r.links; lnk.names = lnk.names || {}; lnkSaved = JSON.stringify(lnk); renderLinks();
      if (tab === 'links') { msg(t.lnkSaved, 'ok'); return; }
    }
    await api('/api/save', { id: current.id, settings: draft });
    current.settings = JSON.parse(JSON.stringify(draft)); saved = JSON.stringify(draft);
    await stopCal();
    msg(t.saved, 'ok');
  } catch (e) { msg(e.message, 'err'); }
};
$('close').onclick = async () => { if ((!dirty() && !lnkDirty()) || confirm(t.unsaved)) { await stopCal(); window.close(); } };
setInterval(drawBar, 60000);

