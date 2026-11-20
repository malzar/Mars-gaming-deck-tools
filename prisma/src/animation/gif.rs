//! GIF propio de arranque o de cierre, por todo el deck (también por las
//! pantallas de las ruedas). Se sigue el reloj del GIF: si el deck no da para
//! todos los fotogramas, se saltan.

use std::{
    io::Cursor,
    path::PathBuf,
    time::{Duration, Instant},
};

use image::{AnimationDecoder, DynamicImage, RgbImage, codecs::gif::GifDecoder, imageops::FilterType};
use mirajazz::{device::Device, error::MirajazzError};

use super::screen::{Layout, draw_canvas, pace};
use crate::{family::Model, settings::Calibration};

/// Límite de un GIF propio
const GIF_MAX_FRAMES: usize = 150;
const GIF_MAX_TIME: Duration = Duration::from_secs(10);

/// Dónde se guarda el GIF propio de un deck ("startup" o "shutdown")
pub fn gif_path(id: &str, which: &str) -> Option<PathBuf> {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    let which = if which == "shutdown" { "shutdown" } else { "startup" };
    Some(crate::app_dir()?.join("animations").join(format!("{safe}-{which}.gif")))
}

/// Comprueba que los bytes son un GIF que se puede reproducir
pub fn validate_gif(bytes: &[u8]) -> Result<usize, String> {
    let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(|e| crate::i18n::tf("error.gif_invalid", &[&e]))?;
    let n = decoder.into_frames().take(GIF_MAX_FRAMES).filter(|f| f.is_ok()).count();
    if n == 0 { Err(crate::i18n::t("error.gif_no_frames").into()) } else { Ok(n) }
}

pub(super) async fn play_gif(device: &Device, model: &Model, cal: &Calibration, id: &str, closing: bool) -> Result<usize, MirajazzError> {
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
                let ms = num.checked_div(den).unwrap_or(100);
                (DynamicImage::ImageRgba8(f.into_buffer()).to_rgb8(), Duration::from_millis(ms.max(40) as u64))
            })
            .collect()
    })
    .await
    .unwrap_or_default();

    let l = Layout::of(model);
    let n = (l.rows * l.cols) as usize;
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
        let canvas = DynamicImage::ImageRgb8(frame).resize_to_fill(l.w, l.h(), FilterType::Triangle);
        draw_canvas(device, model, cal, &l, &canvas).await?;
        device.flush().await?;
        shown += 1;

        if start.elapsed() > GIF_MAX_TIME {
            break;
        }
        // Las imágenes del GIF pesan más que un color liso: el doble de margen
        pace(model, tick, (n + l.zones as usize) * 2, gif_clock.saturating_sub(at)).await;
    }
    Ok(shown)
}
