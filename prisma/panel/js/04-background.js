// ------------------------------------------------------------------ fondo --
let bgImg = null, bgFit = 'cover', bgSrc = 'image';

function drawBackground() {
  const c = $('bgCanvas'), x = c.getContext('2d'), W = c.width, H = c.height;
  const useImage = bgSrc === 'image';
  // Imagen: el color solo se ve en los bordes al encajar. Color plano: solo el color.
  x.fillStyle = useImage && !bgImg ? '#000000' : $('bgColor').value; x.fillRect(0, 0, W, H);
  if (useImage && bgImg) {
    const k = (bgFit === 'cover' ? Math.max : Math.min)(W / bgImg.width, H / bgImg.height);
    const w = bgImg.width * k, h = bgImg.height * k;
    x.imageSmoothingQuality = 'high';
    x.drawImage(bgImg, (W - w) / 2, (H - h) / 2, w, h);
  }
  document.querySelectorAll('[data-src]').forEach((b) => b.classList.toggle('on', b.dataset.src === bgSrc));
  document.querySelectorAll('[data-fit]').forEach((b) => b.classList.toggle('on', b.dataset.fit === bgFit));
  $('bgImageOpts').classList.toggle('hidden', !useImage);
  $('bgColorBox').classList.toggle('hidden', useImage && bgFit !== 'contain');
  $('bgColorLbl').textContent = useImage ? t.bgFill : t.bgColor;
  $('bgApply').disabled = useImage && !bgImg;
  $('bgApply').title = $('bgApply').disabled ? t.bgNoImage : '';
}

function loadImage(src) {
  return new Promise((ok, ko) => { const i = new Image(); i.onload = () => ok(i); i.onerror = ko; i.src = src; });
}

async function loadBackground() {
  const [w, h] = current.screen || [854, 480];
  $('bgCanvas').width = w; $('bgCanvas').height = h; $('bgSize').textContent = w + ' × ' + h;
  bgImg = null; bgFit = 'cover'; bgSrc = 'image';
  if (current.has_background) {
    try { bgImg = await loadImage('/api/background?id=' + encodeURIComponent(current.id) + '&t=' + Date.now()); } catch (_) { bgImg = null; }
  }
  drawBackground();
}

$('bgPick').onclick = () => $('bgFile').click();
$('bgFile').onchange = async (e) => {
  const f = e.target.files[0]; if (!f) return;
  const url = URL.createObjectURL(f);
  try { bgImg = await loadImage(url); drawBackground(); } catch (_) { msg('Imagen no válida', 'err'); }
  e.target.value = '';
};
document.querySelectorAll('[data-fit]').forEach((b) => b.onclick = () => { bgFit = b.dataset.fit; drawBackground(); });
document.querySelectorAll('[data-src]').forEach((b) => b.onclick = () => { bgSrc = b.dataset.src; drawBackground(); });
$('bgColor').oninput = drawBackground;
$('bgApply').onclick = async () => {
  const b = $('bgApply'); b.disabled = true; msg(t.bgUploading, 'wait');
  try {
    const r = await api('/api/background', { id: current.id, image: $('bgCanvas').toDataURL('image/png'), fix: $('bgFix').value });
    if (r.error) throw new Error(r.error);
    current.has_background = true; msg(t.bgDone, 'ok');
  } catch (e) { msg(e.message, 'err'); }
  b.disabled = false;
};

