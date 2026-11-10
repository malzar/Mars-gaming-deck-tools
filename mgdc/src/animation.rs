//! Animaciones de arranque y de cierre.
//!
//! Se pintan en las teclas (no en el fondo: el fondo se graba en la memoria del
//! deck, tarda segundos y desgastaría la memoria). Cada fotograma calcula un
//! color por tecla y solo reenvía las teclas que cambian.

use std::{
    collections::HashSet,
    io::Cursor,
    sync::LazyLock,
    path::PathBuf,
    time::{Duration, Instant},
};

use image::{AnimationDecoder, DynamicImage, Rgb, RgbImage, codecs::gif::GifDecoder, imageops::FilterType};
use mirajazz::{device::Device, error::MirajazzError};
use tokio::sync::Mutex;

use crate::{
    family::Model,
    settings::{AnimKind, Animation, Calibration, parse_hex},
};

/// Decks con una animación en marcha: mientras tanto se ignoran las imágenes
/// que manda OpenDeck (se repintan todas al terminar)
static BUSY: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub async fn busy(id: &str) -> bool {
    BUSY.lock().await.contains(id)
}

pub async fn set_busy(id: &str, on: bool) {
    let mut b = BUSY.lock().await;
    if on { b.insert(id.to_string()); } else { b.remove(id); }
}

/// Tiempo mínimo entre fotogramas (unos 16 por segundo como mucho)
const FRAME: Duration = Duration::from_millis(60);
/// Límite de un GIF propio
const GIF_MAX_FRAMES: usize = 150;
const GIF_MAX_TIME: Duration = Duration::from_secs(10);

/// Color de una paleta en la posición `t` (0..1, cíclica), interpolando
pub fn sample(colors: &[[u8; 3]], t: f32) -> [u8; 3] {
    match colors.len() {
        0 => [255, 255, 255],
        1 => colors[0],
        n => {
            let f = t.rem_euclid(1.0) * n as f32;
            let i = (f.floor() as usize).min(n - 1);
            let k = f - i as f32;
            let (a, b) = (colors[i], colors[(i + 1) % n]);
            [0, 1, 2].map(|j| (a[j] as f32 + (b[j] as f32 - a[j] as f32) * k).round() as u8)
        }
    }
}

pub fn scale(c: [u8; 3], a: f32) -> [u8; 3] {
    let a = a.clamp(0.0, 1.0);
    c.map(|v| (v as f32 * a).round() as u8)
}

fn solid(c: [u8; 3]) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_pixel(8, 8, Rgb(c)))
}

/// Dónde se guarda el GIF propio de un deck ("startup" o "shutdown")
pub fn gif_path(id: &str, which: &str) -> Option<PathBuf> {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    let which = if which == "shutdown" { "shutdown" } else { "startup" };
    Some(dirs::config_dir()?.join("mgdc").join("animations").join(format!("{safe}-{which}.gif")))
}

/// Comprueba que los bytes son un GIF que se puede reproducir
pub fn validate_gif(bytes: &[u8]) -> Result<usize, String> {
    let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(|e| format!("GIF no válido: {e}"))?;
    let n = decoder.into_frames().take(GIF_MAX_FRAMES).filter(|f| f.is_ok()).count();
    if n == 0 { Err("El GIF no tiene fotogramas".into()) } else { Ok(n) }
}

/// Espera lo que necesita el deck tras enviar `keys` teclas (y al menos `min`)
async fn pace(model: &Model, since: Instant, keys: usize, min: Duration) {
    let (base, per_key) = model.anim_pacing();
    let need = Duration::from_millis(base + per_key * keys as u64).max(min);
    let spent = since.elapsed();
    if spent < need {
        tokio::time::sleep(need - spent).await;
    }
}

/// Reproduce una animación. `closing` = animación de cierre (va "al revés").
/// `brightness` es el brillo normal de la pantalla, que el fundido usa y restaura.
pub async fn play(
    device: &Device,
    model: &Model,
    id: &str,
    anim: &Animation,
    closing: bool,
    brightness: u8,
) -> Result<(), MirajazzError> {
    let start = Instant::now();
    // Las animaciones también respetan la calibración de iconos
    let cal = crate::settings::for_device(id).await.calibration;
    let frames = match anim.kind {
        AnimKind::None => return Ok(()),
        AnimKind::Gif => play_gif(device, model, &cal, id, closing).await?,
        AnimKind::Chase => play_steps(device, model, &cal, anim, chase_steps(model), chase_steps(model)).await?,
        AnimKind::Arrow => play_steps(device, model, &cal, anim, arrow_steps(model), arrow_steps(model)).await?,
        // Al apagar "igual", la aleatoria usa otro orden al azar
        AnimKind::Random => play_steps(device, model, &cal, anim, random_steps(model), random_steps(model)).await?,
        AnimKind::Snake => play_steps(device, model, &cal, anim, snake_steps(model), snake_steps(model)).await?,
        AnimKind::Bounce => play_steps(device, model, &cal, anim, bounce_steps(model), bounce_steps(model)).await?,
        AnimKind::Meet => play_steps(device, model, &cal, anim, meet_steps(model), meet_steps(model)).await?,
        AnimKind::Rain => play_steps(device, model, &cal, anim, rain_steps(model), rain_steps(model)).await?,
        AnimKind::Windmill => play_steps(device, model, &cal, anim, windmill_steps(model), windmill_steps(model)).await?,
        AnimKind::Cross => play_steps(device, model, &cal, anim, cross_steps(model), cross_steps(model)).await?,
        AnimKind::Swing => play_steps(device, model, &cal, anim, swing_steps(model), swing_steps(model)).await?,
        // Retiradas: no hacen nada
        AnimKind::Rows | AnimKind::Spiral => 0,
        AnimKind::Fade => play_fade(device, model, &cal, anim, closing, brightness).await?,
        _ => play_effect(device, model, &cal, anim, closing).await?,
    };
    log::info!(
        "Animación {:?} ({}) en {id}: {frames} fotogramas en {} ms",
        anim.kind,
        if closing { "cierre" } else { "arranque" },
        start.elapsed().as_millis()
    );
    Ok(())
}

fn palette(anim: &Animation) -> Vec<[u8; 3]> {
    let mut colors: Vec<[u8; 3]> = anim.colors.iter().map(|c| parse_hex(c)).collect();
    if colors.is_empty() {
        colors.push([255, 255, 255]);
    }
    colors
}

/// Ola y tecla a tecla: solo se reenvían las teclas que cambian
async fn play_effect(
    device: &Device,
    model: &Model,
    cal: &Calibration,
    anim: &Animation,
    closing: bool,
) -> Result<usize, MirajazzError> {
    let (rows, cols) = model.grid();
    let (rows_f, cols_f) = (rows as f32, cols as f32);
    let n = (rows as usize) * (cols as usize);
    let colors = palette(anim);
    let levels = model.anim_levels();
    let duration = Duration::from_millis(anim.duration_ms.clamp(300, 8000) as u64);
    let mut last: Vec<Option<[u8; 3]>> = vec![None; n];
    let start = Instant::now();
    let mut frames = 0;

    loop {
        let tick = Instant::now();
        let p = (start.elapsed().as_secs_f32() / duration.as_secs_f32()).min(1.0);
        let mut sent = 0;

        for y in 0..rows {
            for x in 0..cols {
                let i = (y as usize) * (cols as usize) + x as usize;
                let (xf, yf) = (x as f32, y as f32);
                let a = match anim.kind {
                    AnimKind::Wave => {
                        let f = p * (cols_f + rows_f + 2.0) - (xf + yf * 0.7);
                        let a = (1.0 - (f - 1.0).abs()).max(0.0);
                        (a * levels).round() / levels
                    }
                    _ => {
                        let lit = (i as f32) < p * (n as f32 + 0.999);
                        if lit != closing { 1.0 } else { 0.0 }
                    }
                };
                // El color de cada tecla es fijo durante la animación: menos cambios
                let base = sample(&colors, (xf + yf) / (cols_f + rows_f));
                let c = scale(base, a);
                if last[i] != Some(c) {
                    last[i] = Some(c);
                    model.draw_key(device, i as u8, model.calibrate(cal, i as u8, false, solid(c))).await?;
                    sent += 1;
                }
            }
        }
        if sent > 0 {
            device.flush().await?;
            frames += 1;
        }

        if p >= 1.0 {
            break;
        }
        pace(model, tick, sent, FRAME).await;
    }
    Ok(frames)
}

/// Un paso de animación: las teclas (posiciones de OpenDeck) encendidas en ese momento
type Step = Vec<u8>;

/// Lineal: 0, 1, 2… hasta la última, de una en una
fn chase_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    (0..rows * cols).map(|k| vec![k]).collect()
}

// ----- Animaciones ligeras: como mucho 2 teclas encendidas por paso, así que
// sin estela cambian 4 teclas por paso como máximo (2 se encienden, 2 se apagan)

/// Serpiente: fila 0 de izquierda a derecha, fila 1 de derecha a izquierda…
fn snake_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    (0..rows)
        .flat_map(|r| {
            let cs: Vec<u8> = if r % 2 == 0 { (0..cols).collect() } else { (0..cols).rev().collect() };
            cs.into_iter().map(move |c| vec![r * cols + c])
        })
        .collect()
}

/// Orden de las teclas en espiral (borde exterior en el sentido del reloj, y hacia dentro).
/// Lo usa el Molino para recorrer los anillos.
fn spiral_order(rows: u8, cols: u8) -> Vec<u8> {
    let (mut top, mut left) = (0i32, 0i32);
    let (mut bottom, mut right) = (rows as i32 - 1, cols as i32 - 1);
    let mut v = vec![];
    while top <= bottom && left <= right {
        for c in left..=right { v.push((top, c)); }
        for r in top + 1..=bottom { v.push((r, right)); }
        if top < bottom { for c in (left..right).rev() { v.push((bottom, c)); } }
        if left < right { for r in (top + 1..bottom).rev() { v.push((r, left)); } }
        top += 1; left += 1; bottom -= 1; right -= 1;
    }
    v.into_iter().map(|(r, c)| (r * cols as i32 + c) as u8).collect()
}

/// Rebote: una tecla que va en diagonal y rebota contra los bordes. En
/// diagonal solo pisa la mitad de las teclas (como los alfiles en el ajedrez),
/// así que al terminar esa mitad sigue desde la tecla de al lado con la otra.
fn bounce_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    let (rows, cols) = (rows as i32, cols as i32);
    let total = (rows * cols) as usize;
    let mut seen = std::collections::HashSet::new();
    let mut v = vec![];
    for (mut r, mut c) in [(0i32, 0i32), (0, 1.min(cols - 1))] {
        let (mut dr, mut dc) = (1i32, 1i32);
        for _ in 0..4 * total {
            let k = (r * cols + c) as u8;
            v.push(vec![k]);
            seen.insert(k);
            // Esta mitad ya está completa
            let parity = (r + c) % 2;
            let half = (0..rows * cols).filter(|i| (i / cols + i % cols) % 2 == parity).count();
            let done = seen.iter().filter(|&&i| (i as i32 / cols + i as i32 % cols) % 2 == parity).count();
            if done == half || seen.len() == total {
                break;
            }
            if rows > 1 && !(0..rows).contains(&(r + dr)) { dr = -dr; }
            if !(0..cols).contains(&(c + dc)) { dc = -dc; }
            if rows > 1 { r += dr; }
            c += dc;
        }
        if seen.len() == total {
            break;
        }
    }
    v
}

/// Encuentro: una tecla sale del principio y otra del final y se cruzan en el centro
fn meet_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    let n = rows as usize * cols as usize;
    (0..n.div_ceil(2))
        .map(|i| {
            let (a, b) = (i as u8, (n - 1 - i) as u8);
            if a == b { vec![a] } else { vec![a, b] }
        })
        .collect()
}

/// Lluvia: gotas que caen por columnas al azar; cada gota empieza cuando la
/// anterior va por la penúltima fila, así nunca hay más de 2 a la vez
fn rain_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    let mut order: Vec<u8> = (0..cols).collect();
    let mut rng = Rng::new();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i + 1));
    }
    let gap = (rows as usize).saturating_sub(1).max(1);
    let total = gap * (order.len() - 1) + rows as usize;
    (0..total)
        .map(|t| {
            order
                .iter()
                .enumerate()
                .filter_map(|(d, &c)| {
                    let row = t as i64 - (d * gap) as i64;
                    (0..rows as i64).contains(&row).then(|| row as u8 * cols + c)
                })
                .collect()
        })
        .collect()
}

/// Anillos de la cuadrícula, de fuera hacia dentro, cada uno en el sentido del reloj
fn rings(rows: u8, cols: u8) -> Vec<Vec<u8>> {
    let order = spiral_order(rows, cols);
    let (mut r, mut c) = (rows as usize, cols as usize);
    let mut out = vec![];
    let mut i = 0;
    while r > 0 && c > 0 && i < order.len() {
        let len = if r == 1 || c == 1 { r * c } else { 2 * (r + c) - 4 };
        out.push(order[i..(i + len).min(order.len())].to_vec());
        i += len;
        r = r.saturating_sub(2);
        c = c.saturating_sub(2);
    }
    out
}

/// Molino: dos teclas opuestas dan una vuelta entera por el borde y luego
/// otra por cada anillo de dentro, así pasa por todas las teclas
fn windmill_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    let mut v = vec![];
    for ring in rings(rows, cols) {
        let l = ring.len();
        if l == 1 {
            v.push(vec![ring[0]]);
            continue;
        }
        for i in 0..l {
            let (a, b) = (ring[i], ring[(i + l / 2) % l]);
            v.push(if a == b { vec![a] } else { vec![a, b] });
        }
    }
    v
}

/// Cruce: dos teclas en diagonal, una desde arriba y otra desde abajo, que
/// rebotan en espejo y se cruzan en la fila del centro. A la ida pisan la
/// mitad de las teclas; a la vuelta van desfasadas una fila y pisan la otra mitad.
fn cross_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    let last = rows as i32 - 1;
    let period = (2 * last).max(1);
    let row = |t: i32| {
        let p = t.rem_euclid(period);
        if p <= last { p } else { period - p }
    };
    let pair = |r: i32, c: i32| {
        let a = (r * cols as i32 + c) as u8;
        let b = ((last - r) * cols as i32 + c) as u8;
        if a == b { vec![a] } else { vec![a, b] }
    };
    let ida = (0..cols as i32).map(|c| pair(row(c), c));
    let vuelta = (0..cols as i32).rev().map(|c| pair(row(c + 1), c));
    ida.chain(vuelta).collect()
}

/// Vaivén: una pareja (arriba y abajo de la misma columna) va hacia la
/// derecha y vuelve por la fila del centro, dos teclas cada vez
fn swing_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    let last = rows - 1;
    let mut v: Vec<Step> = (0..cols)
        .map(|c| if last == 0 { vec![c] } else { vec![c, last * cols + c] })
        .collect();
    for r in 1..last {
        let mut c = cols as i32 - 1;
        while c >= 0 {
            let mut step = vec![(r * cols) as i32 + c];
            if c > 0 {
                step.push((r * cols) as i32 + c - 1);
            }
            v.push(step.into_iter().map(|k| k as u8).collect());
            c -= 2;
        }
    }
    v
}

/// Flecha: la fila del centro va delante y las demás se retrasan una columna
/// por cada fila de distancia. En el MSD-ONE (3×6): 6 → 7,0,12 → 8,1,13 → … → 5,17.
fn arrow_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    let mid = (rows as i32 - 1) / 2;
    let max_lag = (0..rows as i32).map(|r| (r - mid).abs()).max().unwrap_or(0);
    (0..cols as i32 + max_lag)
        .map(|k| {
            (0..rows as i32)
                .filter_map(|r| {
                    let c = k - (r - mid).abs();
                    (0..cols as i32).contains(&c).then(|| (r * cols as i32 + c) as u8)
                })
                .collect()
        })
        .collect()
}

/// Aleatorio: todas las teclas en orden al azar, de una en una, sin repetir
fn random_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    let mut keys: Vec<u8> = (0..rows * cols).collect();
    let mut rng = Rng::new();
    for i in (1..keys.len()).rev() {
        keys.swap(i, rng.below(i + 1));
    }
    keys.into_iter().map(|k| vec![k]).collect()
}

/// Generador pseudoaleatorio sencillo (xorshift), sin dependencias
struct Rng(u64);

impl Rng {
    fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Rng(seed | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Color vivo al azar (tono aleatorio, saturación y brillo al máximo)
fn random_color(rng: &mut Rng) -> [u8; 3] {
    let h = (rng.next() % 360) as f32 / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

/// Estela: brillo de una tecla en cada paso desde que deja de estar encendida.
/// Con `n` pasos baja en línea recta hasta apagarse; con 3, por ejemplo:
/// encendida → 75 % → 50 % → 25 % → apagada.
pub fn trail_levels(n: u8) -> Vec<f32> {
    let n = n.min(3) as usize;
    (0..=n).map(|i| 1.0 - i as f32 / (n + 1) as f32).collect()
}

/// Brillo de una tecla en un paso: `Some(i)` = `levels[i]`, `None` = apagada
type Level = Option<usize>;

/// Convierte los pasos (teclas encendidas) en cambios con estela: lo que deja
/// de estar encendido va bajando un escalón de `levels` por paso hasta
/// apagarse. Al final se añaden pasos para que también se apague la última estela.
fn trail_frames(steps: &[Step], levels: usize) -> Vec<Vec<(u8, Level)>> {
    use std::collections::HashMap;
    let mut state: HashMap<u8, usize> = HashMap::new();
    let mut frames = vec![];
    let empty: Step = vec![];
    for step in steps.iter().chain(std::iter::repeat_n(&empty, levels)) {
        let mut changes = vec![];
        let mut keys: Vec<u8> = state.keys().copied().chain(step.iter().copied()).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            let before = state.get(&k).copied();
            let after = if step.contains(&k) {
                Some(0)
            } else {
                before.map(|i| i + 1).filter(|&i| i < levels)
            };
            if after != before {
                changes.push((k, after));
            }
            match after {
                Some(i) => { state.insert(k, i); }
                None => { state.remove(&k); }
            }
        }
        if !changes.is_empty() {
            frames.push(changes);
        }
    }
    frames
}

/// Espejo izquierda-derecha: la columna c pasa a ser la última menos c
fn mirror(model: &Model, steps: Vec<Step>) -> Vec<Step> {
    let (_, cols) = model.grid();
    steps
        .into_iter()
        .map(|s| s.into_iter().map(|k| (k / cols) * cols + (cols - 1 - k % cols)).collect())
        .collect()
}

/// Modo fijo: las teclas se encienden según los pasos y se quedan encendidas.
/// Al final se apagan todas a la vez o, con `off_steps`, siguiendo ese recorrido.
fn fixed_frames(steps: &[Step], off_steps: Option<&[Step]>) -> Vec<Vec<(u8, Level)>> {
    let mut lit: Vec<u8> = vec![];
    let mut frames = vec![];
    for step in steps {
        let on: Vec<(u8, Level)> = step.iter().filter(|k| !lit.contains(k)).map(|&k| (k, Some(0))).collect();
        lit.extend(on.iter().map(|&(k, _)| k));
        if !on.is_empty() {
            frames.push(on);
        }
    }
    match off_steps {
        Some(off) => {
            for step in off {
                let changes: Vec<(u8, Level)> = step.iter().filter(|k| lit.contains(k)).map(|&k| (k, None)).collect();
                lit.retain(|k| !step.contains(k));
                if !changes.is_empty() {
                    frames.push(changes);
                }
            }
            // Por si el recorrido de apagado no pasa por alguna tecla
            if !lit.is_empty() {
                frames.push(lit.iter().map(|&k| (k, None)).collect());
            }
        }
        None if !lit.is_empty() => frames.push(lit.iter().map(|&k| (k, None)).collect()),
        None => {}
    }
    frames
}

/// Reproduce una lista de pasos con estela. En cada paso solo se envían las
/// teclas que cambian (se encienden, bajan al 50 % o se apagan), y se espera
/// lo que necesita el deck según cuántas han cambiado.
async fn play_steps(
    device: &Device,
    model: &Model,
    cal: &Calibration,
    anim: &Animation,
    steps: Vec<Step>,
    off_steps: Vec<Step>,
) -> Result<usize, MirajazzError> {
    // Invertir: en espejo, de derecha a izquierda
    let (steps, off_steps) = if anim.reverse {
        (mirror(model, steps), mirror(model, off_steps))
    } else {
        (steps, off_steps)
    };
    let colors = palette(anim);
    let mut rng = Rng::new();
    let (base, per_key) = model.anim_pacing();
    let levels = trail_levels(anim.trail);
    let frames = if anim.fixed {
        let same = anim.fixed_off == crate::settings::FixedOff::Same;
        fixed_frames(&steps, same.then_some(off_steps.as_slice()))
    } else {
        trail_frames(&steps, levels.len())
    };
    let n = frames.len().max(1);
    let even = Duration::from_millis(anim.duration_ms.clamp(300, 8000) as u64) / n as u32;
    // Color de cada tecla encendida, para que su estela sea del mismo color
    let mut color_of: std::collections::HashMap<u8, [u8; 3]> = std::collections::HashMap::new();
    let mut slowest = 0;

    let max_send = model.max_changes_per_send();
    for (i, frame) in frames.iter().enumerate() {
        // Si un paso cambia más teclas de las que aguanta el deck, se reparte en varios envíos
        let chunks: Vec<&[(u8, Level)]> = frame.chunks(max_send.max(1)).collect();
        let share = even / chunks.len() as u32;
        for changes in chunks {
            let t = Instant::now();
            for &(k, level) in changes {
                match level {
                    None => {
                        model.clear_key(device, k, false).await?;
                        color_of.remove(&k);
                    }
                    Some(0) => {
                        let c = if anim.random_colors {
                            random_color(&mut rng)
                        } else {
                            sample(&colors, i as f32 / steps.len().max(1) as f32)
                        };
                        color_of.insert(k, c);
                        model.draw_key(device, k, model.calibrate(cal, k, false, solid(c))).await?;
                    }
                    Some(t) => {
                        let c = scale(*color_of.get(&k).unwrap_or(&[255, 255, 255]), levels[t]);
                        model.draw_key(device, k, model.calibrate(cal, k, false, solid(c))).await?;
                    }
                }
            }
            device.flush().await?;

            let wait = share.max(Duration::from_millis(base + per_key * changes.len() as u64));
            slowest = slowest.max(wait.as_millis());
            if let Some(rest) = wait.checked_sub(t.elapsed()) {
                tokio::time::sleep(rest).await;
            }
        }
    }
    log::info!("Animación {:?}: {n} pasos ({}), hasta {slowest} ms por paso", anim.kind, if anim.fixed { format!("fija, apagado {:?}", anim.fixed_off) } else { format!("estela de {}", levels.len() - 1) });
    Ok(n)
}

/// Fundido con el brillo de la pantalla: un solo envío de teclas y luego solo
/// órdenes de brillo, que son muy ligeras. Sale suave en todos los decks.
async fn play_fade(
    device: &Device,
    model: &Model,
    cal: &Calibration,
    anim: &Animation,
    closing: bool,
    brightness: u8,
) -> Result<usize, MirajazzError> {
    let b = brightness.max(10) as f32;
    let duration = Duration::from_millis(anim.duration_ms.clamp(300, 8000) as u64);
    let mut frames = 0;

    if !closing {
        // Teclas con los colores de la paleta, con la pantalla a 0
        device.set_brightness(0).await?;
        let (rows, cols) = model.grid();
        let colors = palette(anim);
        let tick = Instant::now();
        for y in 0..rows {
            for x in 0..cols {
                let c = sample(&colors, (x as f32 + y as f32) / (cols as f32 + rows as f32));
                model.draw_key(device, y * cols + x, model.calibrate(cal, y * cols + x, false, solid(c))).await?;
            }
        }
        device.flush().await?;
        pace(model, tick, (rows * cols) as usize, Duration::ZERO).await;
    }

    let start = Instant::now();
    loop {
        let p = (start.elapsed().as_secs_f32() / duration.as_secs_f32()).min(1.0);
        // Arranque: sube en el 60 % del tiempo y baja en el resto. Cierre: baja.
        let level = if closing {
            1.0 - p
        } else if p < 0.6 {
            p / 0.6
        } else {
            1.0 - (p - 0.6) / 0.4
        };
        device.set_brightness((b * level).round() as u8).await?;
        frames += 1;
        if p >= 1.0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    Ok(frames)
}

async fn play_gif(device: &Device, model: &Model, cal: &Calibration, id: &str, closing: bool) -> Result<usize, MirajazzError> {
    let which = if closing { "shutdown" } else { "startup" };
    let Some(bytes) = gif_path(id, which).and_then(|p| std::fs::read(p).ok()) else {
        log::warn!("No hay GIF de {which} para {id}");
        return Ok(0);
    };

    // Decodificar fuera del hilo asíncrono
    let frames = tokio::task::spawn_blocking(move || -> Vec<(RgbImage, Duration)> {
        let Ok(decoder) = GifDecoder::new(Cursor::new(bytes)) else { return vec![] };
        decoder
            .into_frames()
            .take(GIF_MAX_FRAMES)
            .filter_map(|f| f.ok())
            .map(|f| {
                let (num, den) = f.delay().numer_denom_ms();
                let ms = if den == 0 { 100 } else { num / den };
                (DynamicImage::ImageRgba8(f.into_buffer()).to_rgb8(), Duration::from_millis(ms.max(40) as u64))
            })
            .collect()
    })
    .await
    .unwrap_or_default();

    let (rows, cols) = model.grid();
    let k = model.key_size();
    let (w, h) = (cols as u32 * k, rows as u32 * k);
    let n = (rows as usize) * (cols as usize);
    let start = Instant::now();
    // Se sigue el reloj del GIF: si el deck no da para todos los fotogramas, se saltan
    let mut gif_clock = Duration::ZERO;
    let mut shown = 0;

    for (frame, delay) in frames {
        let at = gif_clock;
        gif_clock += delay;
        if start.elapsed() > gif_clock && gif_clock < GIF_MAX_TIME {
            continue;
        }
        let tick = Instant::now();
        let canvas = DynamicImage::ImageRgb8(frame).resize_to_fill(w, h, FilterType::Triangle);
        for y in 0..rows as u32 {
            for x in 0..cols as u32 {
                let tile = canvas.crop_imm(x * k, y * k, k, k);
                let pos = (y * cols as u32 + x) as u8;
                model.draw_key(device, pos, model.calibrate(cal, pos, false, tile)).await?;
            }
        }
        device.flush().await?;
        shown += 1;

        if start.elapsed() > GIF_MAX_TIME {
            break;
        }
        // Las imágenes del GIF pesan más que un color liso: el doble de margen
        pace(model, tick, n * 2, gif_clock.saturating_sub(at)).await;
    }
    Ok(shown)
}

/// Final del cierre: teclas en negro con el fondo a la vista, o deck apagado
pub async fn finish_closing(device: &Device, end_with_background: bool, brightness: u8) -> Result<(), MirajazzError> {
    if end_with_background {
        clear(device).await?;
        device.set_brightness(brightness).await
    } else {
        device.shutdown().await
    }
}

/// Deja todas las teclas en negro (se ve el fondo del deck)
pub async fn clear(device: &Device) -> Result<(), MirajazzError> {
    device.clear_all_button_images().await?;
    device.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flecha_y_filas_en_el_msd_one() {
        let one = Model::from_vid_pid(0x0b00, 0x1000).unwrap();
        let a = arrow_steps(&one);
        assert_eq!(a[0], vec![6]);
        assert_eq!(a[1], vec![0, 7, 12]);
        assert_eq!(a[2], vec![1, 8, 13]);
        assert_eq!(a.last().unwrap(), &vec![5, 17]);
        assert_eq!(a.len(), 7);
        // Estela de 3 pasos en la flecha: el 6 pasa por 75 %, 50 %, 25 % y se apaga
        assert_eq!(trail_levels(3), vec![1.0, 0.75, 0.5, 0.25]);
        assert_eq!(trail_levels(1), vec![1.0, 0.5]);
        assert_eq!(trail_levels(10).len(), 4);
        assert_eq!(trail_levels(0), vec![1.0]);
        // Las 6 animaciones ligeras: sin estela, como mucho 4 teclas cambian por paso
        for steps in [snake_steps(&one), bounce_steps(&one), meet_steps(&one), rain_steps(&one), windmill_steps(&one), cross_steps(&one), swing_steps(&one)] {
            let worst = trail_frames(&steps, 1).iter().map(|f| f.len()).max().unwrap();
            assert!(worst <= 4, "{worst} cambios en {steps:?}");
        }
        assert_eq!(spiral_order(3, 6), vec![0, 1, 2, 3, 4, 5, 11, 17, 16, 15, 14, 13, 12, 6, 7, 8, 9, 10]);
        assert_eq!(snake_steps(&one)[6], vec![11]);
        assert_eq!(meet_steps(&one)[0], vec![0, 17]);
        // Molino, Vaivén, Rebote y Cruce pasan por todas las teclas
        let pro = Model::from_vid_pid(0x0b00, 0x1003).unwrap();
        for steps in [
            windmill_steps(&one), swing_steps(&one), bounce_steps(&one), cross_steps(&one),
            windmill_steps(&pro), swing_steps(&pro), bounce_steps(&pro), cross_steps(&pro),
        ] {
            let mut all: Vec<u8> = steps.into_iter().flatten().collect();
            all.sort();
            all.dedup();
            assert_eq!(all.len(), all.last().map_or(0, |&m| m as usize + 1));
        }
        assert_eq!(rings(3, 6)[1], vec![7, 8, 9, 10]);
        assert_eq!(cross_steps(&one)[0], vec![0, 12]);
        assert_eq!(cross_steps(&one)[1], vec![7]);
        assert_eq!(cross_steps(&one)[6], vec![17, 5]);
        assert_eq!(swing_steps(&one)[0], vec![0, 12]);
        // Invertir: la flecha va hacia la izquierda (empieza en el 11, luego 10, 5 y 17)
        let inv = mirror(&one, arrow_steps(&one));
        assert_eq!(inv[0], vec![11]);
        assert_eq!(inv[1], vec![5, 10, 17]);
        assert_eq!(inv.last().unwrap(), &vec![0, 12]);
        assert_eq!(swing_steps(&one)[6], vec![11, 10]);
        let pro = Model::from_vid_pid(0x0b00, 0x1003).unwrap();
        for steps in [snake_steps(&pro), bounce_steps(&pro), meet_steps(&pro), rain_steps(&pro), windmill_steps(&pro), cross_steps(&pro), swing_steps(&pro)] {
            assert!(steps.iter().flatten().all(|&k| k < 10));
        }
        // Fijo: se encienden y se quedan; al final, de golpe o con el mismo recorrido
        let fx = fixed_frames(&a, None);
        assert_eq!(fx.len(), a.len() + 1);
        assert_eq!(fx.last().unwrap().len(), 18);
        let fs = fixed_frames(&a, Some(&a));
        assert_eq!(fs.len(), a.len() * 2);
        assert_eq!(fs[a.len()], vec![(6, None)]);
        assert_eq!(fs.last().unwrap(), &vec![(5, None), (17, None)]);
        let f = trail_frames(&a, 4);
        assert!(f[1].contains(&(6, Some(1))) && f[1].contains(&(7, Some(0))));
        assert!(f[2].contains(&(6, Some(2))) && f[2].contains(&(7, Some(1))) && f[2].contains(&(8, Some(0))));
        assert!(f[3].contains(&(6, Some(3))));
        assert!(f[4].contains(&(6, None)));
        // Al final: 5 y 17 bajan y se apagan
        assert_eq!(f.len(), a.len() + 4);
        assert!(f.last().unwrap().iter().all(|&(_, l)| l.is_none()));
        // Lineal: 0 → 75 % → 50 % → 25 % → apagado
        let l = trail_frames(&chase_steps(&one), 4);
        assert_eq!(l[0], vec![(0, Some(0))]);
        assert_eq!(l[1], vec![(0, Some(1)), (1, Some(0))]);
        assert_eq!(l[4], vec![(0, None), (1, Some(3)), (2, Some(2)), (3, Some(1)), (4, Some(0))]);
        let mut all: Vec<u8> = random_steps(&one).into_iter().flatten().collect();
        all.sort();
        assert_eq!(all, (0..18).collect::<Vec<u8>>());
    }

    #[test]
    fn paleta_interpola_y_da_la_vuelta() {
        let c = [[0, 0, 0], [200, 100, 0]];
        assert_eq!(sample(&c, 0.0), [0, 0, 0]);
        assert_eq!(sample(&c, 0.25), [100, 50, 0]);
        assert_eq!(sample(&c, 0.5), [200, 100, 0]);
        assert_eq!(sample(&c, 1.0), [0, 0, 0]);
        assert_eq!(sample(&[[9, 9, 9]], 0.7), [9, 9, 9]);
    }
}

#[cfg(test)]
#[test]
#[ignore]
fn volcar_pasos() {
    let one = Model::from_vid_pid(0x0b00, 0x1000).unwrap();
    let all = [
        ("Lineal", chase_steps(&one)), ("Flecha", arrow_steps(&one)),
        ("Aleatorio", random_steps(&one)), ("Serpiente", snake_steps(&one)),
        ("Rebote", bounce_steps(&one)), ("Encuentro", meet_steps(&one)), ("Lluvia", rain_steps(&one)),
        ("Molino", windmill_steps(&one)), ("Cruce", cross_steps(&one)), ("Vaivén", swing_steps(&one)),
    ];
    for (name, steps) in all {
        println!("{name}|{:?}", steps);
    }
}
