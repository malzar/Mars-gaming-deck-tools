// --------------------------------------------------------- animaciones --
const KLBL = { none: 'kNone', cascade: 'kCascade', chase: 'kChase', arrow: 'kArrow', rows: 'kRows', random: 'kRandom', snake: 'kSnake', bounce: 'kBounce', meet: 'kMeet', rain: 'kRain', windmill: 'kWindmill', cross: 'kCross', swing: 'kSwing', wave: 'kWave', keys: 'kKeys', fade: 'kFade', gif: 'kGif' };
let gifWhich = null;
// Tipos de animación según el deck (el MSD-ONE solo admite la cascada)
function renderKinds() {
  const kinds = current.anim_kinds || ['none', 'cascade', 'wave', 'keys', 'fade', 'gif'];
  // Desplegable: con tantas animaciones, los botones no caben
  const box = $('animKind');
  box.innerHTML = `<select>${kinds.map((k) => `<option value="${k}">${t[KLBL[k]]}</option>`).join('')}</select>`;
  box.querySelector('select').onchange = (e) => { ed().kind = e.target.value; updateAnims(); };
}
const anims = () => draft.animations;
// Animación cuyos ajustes (duración, estela, invertir, colores) se están tocando
let edit = 'startup';
const other = () => (edit === 'startup' ? 'shutdown' : 'startup');
const ed = () => anims()[edit];
function animColors(cols, pal, rnd = false) { const x = ed(); x.colors = cols.slice(); x.palette = pal; x.random_colors = rnd; }
// Cómo se queda el deck al terminar el cierre (los ajustes antiguos solo tenían fondo/logo/apagado)
const endKind = (a) => a.ending || (a.end_with_logo ? 'logo' : a.end_with_background ? 'background' : 'off');
function fillAnims() {
  const a = anims(), kinds = current.anim_kinds || [];
  renderKinds();
  $('oneNote').classList.toggle('hidden', !(kinds.length && !kinds.includes('wave')));
  $('endWith').value = endKind(a);
  updateAnims();
}
function updateAnims() {
  const a = anims(), x = a[edit], isRnd = !!x.random_colors, isOne = !isRnd && x.palette === '';
  document.querySelectorAll('#animEdit button').forEach((b) => b.classList.toggle('on', b.dataset.ed === edit));
  $('animCopy').textContent = `${t.copyTo} ${t[other()].toLowerCase()}`;
  // El cierre se puede probar aunque no tenga animación: se ve cómo se queda
  $('animTest').disabled = x.kind === 'none' && edit !== 'shutdown';
  // El final solo existe en el cierre; al arrancar el deck se queda en negro
  $('endRow').classList.toggle('hidden', edit !== 'shutdown');
  {
    const box = $('endPhoto'), img = box.querySelector('img');
    box.classList.toggle('hidden', $('endWith').value !== 'photo');
    img.classList.toggle('hidden', !current.photo);
    if (current.photo && img.dataset.for !== current.id + current.photo) { img.src = `/api/animation/photo?id=${encodeURIComponent(current.id)}&t=${Date.now()}`; img.dataset.for = current.id + current.photo; }
  }
  $('animDur').value = x.duration_ms / 1000;
  $('animOne').value = x.colors[0] || '#2f7de1';
  {
    const w = edit, sel = $('animKind').querySelector('select'); if (sel) sel.value = x.kind;
    const box = $('animGif'), img = box.querySelector('img');
    box.classList.toggle('hidden', x.kind !== 'gif');
    const has = current.gifs && current.gifs[w];
    img.classList.toggle('hidden', !has);
    if (has && img.dataset.for !== current.id + w + current.gifs[w]) { img.src = `/api/animation/gif?id=${encodeURIComponent(current.id)}&which=${w}&t=${Date.now()}`; img.dataset.for = current.id + w + current.gifs[w]; }
  }
  // Con "Ninguna" no hay nada que ajustar ni probar
  $('animOpts').classList.toggle('hidden', x.kind === 'none');
  $('animCopy').classList.toggle('hidden', x.kind === 'none');
  $('animDurV').textContent = (+$('animDur').value).toFixed(1).replace('.', ',') + ' s';
  // La estela solo cuenta en las animaciones de pasos
  const trailed = ['chase', 'arrow', 'random', 'snake', 'bounce', 'meet', 'rain', 'windmill', 'cross', 'swing'].includes(x.kind);
  $('animRev').checked = !!x.reverse;
  [$('animRev').closest('span'), $('animRev').closest('.row').querySelector('[data-t=reverse]')].forEach((e) => e.classList.toggle('hidden', !trailed));
  const fixed = !!x.fixed, tr = Math.min(3, Math.max(0, x.trail ?? 3));
  document.querySelectorAll('#animTrail button').forEach((b) => b.classList.toggle('on', fixed ? b.dataset.tr === 'fixed' : b.dataset.tr === String(tr)));
  $('animRev').onchange = (e) => { ed().reverse = e.target.checked; updateAnims(); };
  [$('animTrail').closest('span'), $('animTrail').closest('.row').querySelector('[data-t=trail]')].forEach((e) => e.classList.toggle('hidden', !trailed));
  // Los colores solo cuentan en ola, tecla a tecla y fundido
  const colorful = ['chase', 'arrow', 'random', 'snake', 'bounce', 'meet', 'rain', 'windmill', 'cross', 'swing', 'wave', 'keys', 'fade'].includes(x.kind);

  $('colorsBox').classList.toggle('hidden', !colorful);
  const onlyOne = (current.anim_kinds || []).length && !(current.anim_kinds || []).includes('wave');
  // El MSD-ONE no se apaga al cerrar: fondo o logo
  $('endWith').querySelector('[value=off]').hidden = onlyOne;
  if (onlyOne && $('endWith').value === 'off') $('endWith').value = 'background';
  const cm = isRnd ? 'rnd' : isOne ? 'one' : 'pal';
  document.querySelectorAll('#animCm button').forEach((b) => b.classList.toggle('on', b.dataset.cm === cm));
  $('animOne').classList.toggle('hidden', cm !== 'one'); $('animPal').classList.toggle('hidden', cm !== 'pal');
  palGrid($('animPal'), () => ed().palette, (n, c) => { animColors(c, n); });
}
document.querySelectorAll('#animTrail button').forEach((b) => b.onclick = () => {
  const x = ed();
  if (b.dataset.tr === 'fixed') x.fixed = true; else { x.fixed = false; x.trail = +b.dataset.tr; }
  updateAnims();
});
document.querySelectorAll('#animEdit button').forEach((b) => b.onclick = () => { edit = b.dataset.ed; updateAnims(); });
// Copia duración, estela, invertir y colores a la otra animación (el tipo no)
$('animCopy').onclick = () => {
  const from = ed(), to = anims()[other()];
  for (const k of ['duration_ms', 'trail', 'fixed', 'reverse', 'palette', 'random_colors']) to[k] = from[k];
  to.colors = from.colors.slice();
  msg(`${t.copyTo} ${t[other()].toLowerCase()} ✓`, 'ok');
};
$('animDur').oninput = (e) => { ed().duration_ms = Math.round(+e.target.value * 1000); updateAnims(); };
$('endWith').onchange = (e) => {
  const v = e.target.value, a = anims();
  a.ending = v; a.end_with_logo = v === 'logo'; a.end_with_background = v !== 'off';
  if (v === 'photo' && !current.photo) $('photoFile').click();
  updateAnims();
};
$('endPhoto').querySelector('button').onclick = () => $('photoFile').click();
$('photoFile').onchange = (e) => {
  const f = e.target.files[0]; e.target.value = ''; if (!f) return;
  const r = new FileReader();
  r.onload = async () => {
    try {
      const res = await api('/api/animation/photo', { id: current.id, data: r.result });
      if (res.error) throw new Error(res.error);
      current.photo = Date.now(); msg(t.photoSaved, 'ok'); updateAnims();
    } catch (err) { msg(err.message, 'err'); }
  };
  r.readAsDataURL(f);
};
$('animOne').oninput = (e) => animColors([e.target.value], '');
document.querySelectorAll('#animCm button').forEach((b) => b.onclick = () => {
  if (b.dataset.cm === 'one') animColors([$('animOne').value], '');
  else if (b.dataset.cm === 'rnd') animColors(BUILTIN[0][1], '', true);
  else { const p = BUILTIN[0]; animColors(p[1], p[0]); }
  updateAnims();
});
$('animGif').querySelector('button').onclick = () => { gifWhich = edit; $('gifFile').click(); };
$('gifFile').onchange = (e) => {
  const f = e.target.files[0]; e.target.value = ''; if (!f) return;
  const r = new FileReader();
  r.onload = async () => {
    try {
      const res = await api('/api/animation/gif', { id: current.id, which: gifWhich, data: r.result });
      if (res.error) throw new Error(res.error);
      current.gifs = current.gifs || {}; current.gifs[gifWhich] = Date.now(); msg(t.gifSaved, 'ok'); updateAnims();
    } catch (err) { msg(err.message, 'err'); }
  };
  r.readAsDataURL(f);
};
// Prueba la animación que se está ajustando
$('animTest').onclick = async () => {
  const b = $('animTest'), w = edit, a = anims()[w];
  if (a.kind === 'gif' && !(current.gifs && current.gifs[w])) { msg(t.noGif, 'err'); return; }
  const ending = w === 'shutdown' ? $('endWith').value : null;
  if (ending === 'photo' && !current.photo) { msg(t.noPhoto, 'err'); return; }
  b.disabled = true; msg(t.playing, 'wait');
  try { const r = await api('/api/animation/test', { id: current.id, closing: w === 'shutdown', animation: a, ending }); if (r.error) throw new Error(r.error); msg(''); }
  catch (e) { msg(e.message, 'err'); }
  b.disabled = false;
};

function redrawPals() { if (draft) updateAnims(); }

