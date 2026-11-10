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
    settings::{AnimKind, Animation, parse_hex},
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
    let frames = match anim.kind {
        AnimKind::None => return Ok(()),
        AnimKind::Gif => play_gif(device, model, id, closing).await?,
        AnimKind::Fade => play_fade(device, model, anim, closing, brightness).await?,
        _ => play_effect(device, model, anim, closing).await?,
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
                    model.draw_key(device, i as u8, solid(c)).await?;
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

/// Fundido con el brillo de la pantalla: un solo envío de teclas y luego solo
/// órdenes de brillo, que son muy ligeras. Sale suave en todos los decks.
async fn play_fade(
    device: &Device,
    model: &Model,
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
                model.draw_key(device, y * cols + x, solid(c)).await?;
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

async fn play_gif(device: &Device, model: &Model, id: &str, closing: bool) -> Result<usize, MirajazzError> {
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
                model.draw_key(device, (y * cols as u32 + x) as u8, tile).await?;
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
    fn paleta_interpola_y_da_la_vuelta() {
        let c = [[0, 0, 0], [200, 100, 0]];
        assert_eq!(sample(&c, 0.0), [0, 0, 0]);
        assert_eq!(sample(&c, 0.25), [100, 50, 0]);
        assert_eq!(sample(&c, 0.5), [200, 100, 0]);
        assert_eq!(sample(&c, 1.0), [0, 0, 0]);
        assert_eq!(sample(&[[9, 9, 9]], 0.7), [9, 9, 9]);
    }
}
