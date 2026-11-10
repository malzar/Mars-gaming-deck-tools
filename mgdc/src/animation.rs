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
        AnimKind::Chase => play_steps(device, model, &cal, anim, chase_steps(model)).await?,
        AnimKind::Arrow => play_steps(device, model, &cal, anim, arrow_steps(model)).await?,
        AnimKind::Rows => play_steps(device, model, &cal, anim, row_steps(model)).await?,
        AnimKind::Random => play_steps(device, model, &cal, anim, random_steps(model)).await?,
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

/// Filas: 0-5, luego 6-11 (y se apaga la anterior), y así hasta la última
fn row_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = model.grid();
    (0..rows).map(|r| (0..cols).map(|c| r * cols + c).collect()).collect()
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

/// Estela: brillo de una tecla en los pasos siguientes a apagarse
/// (encendida → 70 % → 30 % → 10 % → apagada)
const TRAIL: [f32; 4] = [1.0, 0.7, 0.3, 0.1];

/// Brillo de una tecla en un paso: `Some(i)` = `TRAIL[i]`, `None` = apagada
type Level = Option<usize>;

/// Convierte los pasos (teclas encendidas) en cambios con estela: lo que deja
/// de estar encendido va bajando por `TRAIL` un escalón por paso hasta
/// apagarse. Al final se añaden pasos para que también se apague la última estela.
fn trail_frames(steps: &[Step]) -> Vec<Vec<(u8, Level)>> {
    use std::collections::HashMap;
    let mut state: HashMap<u8, usize> = HashMap::new();
    let mut frames = vec![];
    let empty: Step = vec![];
    for step in steps.iter().chain(std::iter::repeat_n(&empty, TRAIL.len())) {
        let mut changes = vec![];
        let mut keys: Vec<u8> = state.keys().copied().chain(step.iter().copied()).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            let before = state.get(&k).copied();
            let after = if step.contains(&k) {
                Some(0)
            } else {
                before.map(|i| i + 1).filter(|&i| i < TRAIL.len())
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

/// Reproduce una lista de pasos con estela. En cada paso solo se envían las
/// teclas que cambian (se encienden, bajan al 50 % o se apagan), y se espera
/// lo que necesita el deck según cuántas han cambiado.
async fn play_steps(
    device: &Device,
    model: &Model,
    cal: &Calibration,
    anim: &Animation,
    steps: Vec<Step>,
) -> Result<usize, MirajazzError> {
    let colors = palette(anim);
    let mut rng = Rng::new();
    let (base, per_key) = model.anim_pacing();
    let frames = trail_frames(&steps);
    let n = frames.len().max(1);
    let even = Duration::from_millis(anim.duration_ms.clamp(300, 8000) as u64) / n as u32;
    // Color de cada tecla encendida, para que su estela sea del mismo color
    let mut color_of: std::collections::HashMap<u8, [u8; 3]> = std::collections::HashMap::new();
    let mut slowest = 0;

    for (i, changes) in frames.iter().enumerate() {
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
                    let c = scale(*color_of.get(&k).unwrap_or(&[255, 255, 255]), TRAIL[t]);
                    model.draw_key(device, k, model.calibrate(cal, k, false, solid(c))).await?;
                }
            }
        }
        device.flush().await?;

        let wait = even.max(Duration::from_millis(base + per_key * changes.len() as u64));
        slowest = slowest.max(wait.as_millis());
        if let Some(rest) = wait.checked_sub(t.elapsed()) {
            tokio::time::sleep(rest).await;
        }
    }
    log::info!("Animación {:?}: {n} pasos con estela (70/30/10 %), hasta {slowest} ms por paso", anim.kind);
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
        let r = row_steps(&one);
        assert_eq!(r[1], vec![6, 7, 8, 9, 10, 11]);
        // Estela de la flecha: el 6 pasa por 70 %, 30 %, 10 % y se apaga
        let f = trail_frames(&a);
        assert!(f[1].contains(&(6, Some(1))) && f[1].contains(&(7, Some(0))));
        assert!(f[2].contains(&(6, Some(2))) && f[2].contains(&(7, Some(1))) && f[2].contains(&(8, Some(0))));
        assert!(f[3].contains(&(6, Some(3))));
        assert!(f[4].contains(&(6, None)));
        // Al final: 5 y 17 bajan y se apagan
        assert_eq!(f.len(), a.len() + TRAIL.len());
        assert!(f.last().unwrap().iter().all(|&(_, l)| l.is_none()));
        // Lineal: 0 → 70 % → 30 % → 10 % → apagado
        let l = trail_frames(&chase_steps(&one));
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
