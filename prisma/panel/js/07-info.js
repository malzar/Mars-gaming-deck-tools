// ------------------------------------------------------------------ info --
$('resetDeck').onclick = () => api('/api/reset', { id: current.id }).catch((e) => msg(e.message, 'err'));
$('copyInfo').onclick = async () => {
  const text = infoList().map(([a, b]) => `${a}: ${b}`).join('\n');
  try { await navigator.clipboard.writeText(text); msg(t.copied, 'ok'); } catch (_) { msg(text); }
};

