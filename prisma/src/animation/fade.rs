//! Fundido con el brillo de la pantalla: un solo envío de teclas y luego solo
//! órdenes de brillo, que son muy ligeras. Sale suave en todos los decks.

use std::time::{Duration, Instant};

use mirajazz::{device::Device, error::MirajazzError};

use super::{
    MAX_MS, MIN_MS,
    color::{palette, sample},
    screen::{Screen, anim_grid, pace},
};
use crate::{
    family::Model,
    settings::{Animation, Calibration},
};

pub(super) async fn play_fade(
    device: &Device,
    model: &Model,
    cal: &Calibration,
    anim: &Animation,
    closing: bool,
    brightness: u8,
) -> Result<usize, MirajazzError> {
    let b = brightness.max(10) as f32;
    let duration = Duration::from_millis(anim.duration_ms.clamp(MIN_MS, MAX_MS) as u64);
    let mut frames = 0;

    if !closing {
        // Teclas con los colores de la paleta, con la pantalla a 0
        device.set_brightness(0).await?;
        let (rows, cols) = anim_grid(model);
        let colors = palette(anim);
        let mut screen = Screen::new(device, model, cal, true);
        let tick = Instant::now();
        for y in 0..rows {
            for x in 0..cols {
                let c = sample(&colors, (x as f32 + y as f32) / (cols as f32 + rows as f32));
                screen.set(y * cols + x, Some(c)).await?;
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
