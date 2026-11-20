// ------------------------------------------------------------- arranque --
(async () => {
  try {
    const st = await api('/api/state');
    $('version').textContent = 'v' + st.version;
    devices = st.devices; custom = st.palettes || [];
    if (!devices.length) { $('empty').textContent = t.empty; $('empty').classList.remove('hidden'); return; }
    $('main').classList.remove('hidden');
    select(devices[0].id);
    showTab(tab);
    if (tab === 'links') loadLinks();
  } catch (e) {
    $('empty').textContent = e.message; $('empty').classList.remove('hidden');
  }
})();
