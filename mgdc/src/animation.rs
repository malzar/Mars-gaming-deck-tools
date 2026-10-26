//! Animaciones de arranque y de cierre.
//!
//! Se pintan en las teclas (no en el fondo: el fondo se graba en la memoria del
//! deck, tarda segundos y desgastaría la memoria). Cada fotograma calcula un
//! color por tecla y solo reenvía las teclas que cambian.

use std::{
    io::Cursor,
    path::PathBuf,
    time::{Duration, Instant},
};

use image::{AnimationDecoder, DynamicImage, Rgb, RgbImage, codecs::gif::GifDecoder, imageops::FilterType};
use mirajazz::{device::Device, error::MirajazzError};

use crate::{
    family::Model,
    settings::{AnimKind, Animation, parse_hex},
};

/// Tiempo mínimo entre fotogramas
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

/// Reproduce una animación. `closing` = animación de cierre (va "al revés").
pub async fn play(
    device: &Device,
    model: &Model,
    id: &str,
    anim: &Animation,
    closing: bool,
) -> Result<(), MirajazzError> {
    match anim.kind {
        AnimKind::None => Ok(()),
        AnimKind::Gif => play_gif(device, model, id, closing).await,
        _ => play_effect(device, model, anim, closing).await,
    }
}

async fn play_effect(
    device: &Device,
    model: &Model,
    anim: &Animation,
    closing: bool,
) -> Result<(), MirajazzError> {
    let (rows, cols) = model.grid();
    let (rows_f, cols_f) = (rows as f32, cols as f32);
    let n = (rows as usize) * (cols as usize);
    let mut colors: Vec<[u8; 3]> = anim.colors.iter().map(|c| parse_hex(c)).collect();
    if colors.is_empty() {
        colors.push([255, 255, 255]);
    }
    let duration = Duration::from_millis(anim.duration_ms.clamp(300, 8000) as u64);
    let mut last: Vec<Option<[u8; 3]>> = vec![None; n];
    let start = Instant::now();

    loop {
        let tick = Instant::now();
        let p = (start.elapsed().as_secs_f32() / duration.as_secs_f32()).min(1.0);

        for y in 0..rows {
            for x in 0..cols {
                let i = (y as usize) * (cols as usize) + x as usize;
                let (xf, yf) = (x as f32, y as f32);
                let base = sample(&colors, (xf + yf) / (cols_f + rows_f) + p * 0.5);
                let a = match anim.kind {
                    AnimKind::Wave => {
                        let f = p * (cols_f + rows_f + 2.0) - (xf + yf * 0.7);
                        (1.0 - (f - 1.0).abs()).max(0.0)
                    }
                    AnimKind::Keys => {
                        let lit = (i as f32) < p * (n as f32 + 0.999);
                        if lit != closing { 1.0 } else { 0.0 }
                    }
                    AnimKind::Fade => {
                        // Sube y vuelve a bajar al final del arranque, para dar paso a las teclas
                        if closing { 1.0 - p } else if p < 0.7 { p / 0.7 } else { 1.0 - (p - 0.7) / 0.3 * 0.6 }
                    }
                    _ => 0.0,
                };
                let c = scale(base, a);
                if last[i] != Some(c) {
                    last[i] = Some(c);
                    model.draw_key(device, i as u8, solid(c)).await?;
                }
            }
        }
        device.flush().await?;

        if p >= 1.0 {
            break;
        }
        let spent = tick.elapsed();
        if spent < FRAME {
            tokio::time::sleep(FRAME - spent).await;
        }
    }
    Ok(())
}

async fn play_gif(device: &Device, model: &Model, id: &str, closing: bool) -> Result<(), MirajazzError> {
    let which = if closing { "shutdown" } else { "startup" };
    let Some(bytes) = gif_path(id, which).and_then(|p| std::fs::read(p).ok()) else {
        log::warn!("No hay GIF de {which} para {id}");
        return Ok(());
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
    let start = Instant::now();

    for (frame, delay) in frames {
        let tick = Instant::now();
        let canvas = DynamicImage::ImageRgb8(frame).resize_to_fill(w, h, FilterType::Triangle);
        for y in 0..rows as u32 {
            for x in 0..cols as u32 {
                let tile = canvas.crop_imm(x * k, y * k, k, k);
                model.draw_key(device, (y * cols as u32 + x) as u8, tile).await?;
            }
        }
        device.flush().await?;

        if start.elapsed() > GIF_MAX_TIME {
            break;
        }
        let spent = tick.elapsed();
        if spent < delay {
            tokio::time::sleep(delay - spent).await;
        }
    }
    Ok(())
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
