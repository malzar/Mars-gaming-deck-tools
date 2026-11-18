// ------------------------------------------------------ perfiles enlazados --
let lnk = null, lnkDevs = [], lnkSaved = '', lnkMenu = false;
const lnkDirty = () => lnk && JSON.stringify(lnk) !== lnkSaved;
async function loadLinks() {
  try {
    const r = await api('/api/links');
    lnkDevs = r.devices || [];
    // Al volver a la pestaña se releen los perfiles; lo que aún no se ha guardado se respeta
    if (!lnkDirty()) {
      lnk = r.links; lnkSaved = JSON.stringify(lnk);
      lnk.devices = lnk.devices || []; lnk.links = lnk.links || []; lnk.names = lnk.names || {};
      lnkSaved = JSON.stringify(lnk);
    }
    renderLinks();
  } catch (e) { $('lnkBody').textContent = e.message; }
}
const lnkDev = (id) => lnkDevs.find((d) => d.id === id) || { id, name: (lnk.names || {})[id] || id, connected: false, profiles: [] };
const short = (n) => n.replace(/^Mars Gaming /, '');
function renderLinks() {
  if (!lnk) return;
  $('lnkOn').checked = !!lnk.enabled;
  const cols = lnk.devices, free = lnkDevs.filter((d) => !cols.includes(d.id));
  const menu = lnkMenu ? `<div class="lnkMenu">${free.length ? free.map((d) => `<button data-addd="${esc(d.id)}">${esc(d.name)}<small>${d.profiles.length} ${t.lnkProfiles} · ${d.connected ? t.lnkConn : t.lnkOff}</small></button>`).join('') : `<div style="padding:8px 10px;color:var(--muted)">${t.lnkNoMore}</div>`}</div>` : '';
  if (!cols.length) {
    $('lnkBody').innerHTML = `<div class="lnkEmpty">${t.lnkEmpty}<br><br><span style="position:relative;display:inline-block"><button class="lnkAdd" id="lnkAddD">${t.lnkAddFirst}</button>${menu}</span></div>`;
  } else {
    const head = cols.map((id) => { const d = lnkDev(id); return `<th><div class="colh"><span>${esc(short(d.name))}${d.connected ? '' : `<span class="off">${t.lnkOff}</span>`}</span><button class="rm" data-rmd="${esc(id)}" title="✕">✕</button></div></th>`; }).join('');
    const rows = lnk.links.map((row, r) => '<tr>' + cols.map((id) => {
      const d = lnkDev(id), val = row[id] || '';
      // Un perfil solo puede estar en un enlace
      const used = lnk.links.filter((x, i) => i !== r).map((x) => x[id]).filter(Boolean);
      // Con la jerarquía de la lista de perfiles (subperfiles metidos hacia dentro)
      const tree = d.tree && d.tree.length ? d.tree : d.profiles.map((p) => ({ id: p, name: p, depth: 0 }));
      const opts = [`<option value="">${t.lnkNone}</option>`].concat(tree.map((p) => `<option value="${esc(p.id)}" ${p.id === val ? 'selected' : ''} ${used.includes(p.id) ? 'disabled' : ''}>${'\u00a0\u00a0\u00a0'.repeat(p.depth)}${p.depth ? '└ ' : ''}${esc(p.name)}</option>`));
      if (val && !d.profiles.includes(val)) opts.push(`<option value="${esc(val)}" selected>${esc(val)} (?)</option>`);
      return `<td><select class="${val ? '' : 'none'}" data-r="${r}" data-d="${esc(id)}">${opts.join('')}</select></td>`;
    }).join('') + `<td style="width:30px"><button class="xrow" data-xr="${r}" title="✕">✕</button></td></tr>`).join('');
    $('lnkBody').innerHTML = `<table class="lnk"><tr>${head}<th style="width:90px"><span style="position:relative;display:inline-block"><button class="lnkAdd" id="lnkAddD">${t.lnkAddDeck}</button>${menu}</span></th></tr>${rows}</table>
      <button class="lnkAdd" id="lnkAddR" style="margin-top:10px">${t.lnkAddRow}</button>
      <p style="font-size:12px;color:var(--muted);margin:10px 0 0">${t.lnkNote}</p>`;
  }
  $('lnkAddD').onclick = (e) => { e.stopPropagation(); lnkMenu = !lnkMenu; renderLinks(); };
  $('lnkBody').querySelectorAll('[data-addd]').forEach((b) => b.onclick = () => {
    const d = lnkDev(b.dataset.addd); lnk.devices.push(d.id); lnk.names[d.id] = d.name; lnkMenu = false;
    if (!lnk.links.length) lnk.links.push({});
    renderLinks();
  });
  $('lnkBody').querySelectorAll('[data-rmd]').forEach((b) => b.onclick = () => {
    const id = b.dataset.rmd; lnk.devices = lnk.devices.filter((x) => x !== id); delete lnk.names[id];
    lnk.links.forEach((row) => delete row[id]); renderLinks();
  });
  $('lnkBody').querySelectorAll('select[data-r]').forEach((sel) => sel.onchange = () => {
    const row = lnk.links[+sel.dataset.r]; if (sel.value) row[sel.dataset.d] = sel.value; else delete row[sel.dataset.d]; renderLinks();
  });
  $('lnkBody').querySelectorAll('[data-xr]').forEach((b) => b.onclick = () => { lnk.links.splice(+b.dataset.xr, 1); renderLinks(); });
  const addR = $('lnkAddR'); if (addR) addR.onclick = () => { lnk.links.push({}); renderLinks(); };
}
$('lnkOn').onchange = () => { if (lnk) lnk.enabled = $('lnkOn').checked; };
document.addEventListener('click', () => { if (lnkMenu) { lnkMenu = false; renderLinks(); } });
document.querySelector('#tabs [data-go="links"]').addEventListener('click', loadLinks);

