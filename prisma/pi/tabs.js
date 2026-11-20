// Pestañas de las ventanas de ajustes: botones `[data-tab]` dentro de
// `.tabs` y paneles `[data-pane]` con el mismo nombre. Se recuerda la última
// pestaña abierta de cada ventana (`key`). Las pestañas con `.dialOnly` solo
// salen en las ruedas, y cualquiera se puede ocultar con `hide(nombre, sí/no)`.
window.PrismaTabs = function (root, key) {
  const css = document.createElement('style');
  css.textContent = '.tabs{display:flex;flex-wrap:wrap;gap:2px;margin:0 0 12px;border-bottom:1px solid var(--border)}'
    + '.tabs button{height:28px;border:none;border-bottom:2px solid transparent;border-radius:0;background:none;color:var(--muted);padding:0 10px;font:inherit;cursor:pointer}'
    + '.tabs button.on{color:var(--text);border-bottom-color:#4aa3ff}';
  document.head.appendChild(css);
  const buttons = () => [...root.querySelectorAll('.tabs button[data-tab]')];
  let dial = false;
  const hidden = new Set();
  let tab = '';
  try { tab = localStorage.getItem('prisma.tab.' + key) || ''; } catch (e) { /* sin almacenamiento */ }
  function show() {
    const ok = buttons().filter((b) => (dial || !b.classList.contains('dialOnly')) && !hidden.has(b.dataset.tab)).map((b) => b.dataset.tab);
    if (!ok.includes(tab)) tab = ok[0];
    buttons().forEach((b) => { b.classList.toggle('on', b.dataset.tab === tab); b.style.display = ok.includes(b.dataset.tab) ? '' : 'none'; });
    root.querySelectorAll('[data-pane]').forEach((p) => { p.style.display = p.dataset.pane === tab ? '' : 'none'; });
  }
  buttons().forEach((b) => b.addEventListener('click', () => {
    tab = b.dataset.tab;
    try { localStorage.setItem('prisma.tab.' + key, tab); } catch (e) { /* sin almacenamiento */ }
    show();
  }));
  return { show, setDial(v) { dial = v; show(); }, hide(name, yes) { if (yes) hidden.add(name); else hidden.delete(name); show(); } };
};
