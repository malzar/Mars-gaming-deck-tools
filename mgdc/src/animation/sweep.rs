//! Ola y tecla a tecla: cada fotograma calcula el color de cada tecla y solo
//! se reenvían las que cambian

use std::time::{Duration, Instant};

use mirajazz::{device::Device, error::MirajazzError};

use super::{
    FRAME, MAX_MS, MIN_MS,
    color::{palette, sample, scale},
    screen::{Screen, anim_grid, pace},
};
use crate::{
    family::Model,
    settings::{AnimKind, Animation, Calibration},
};

/// Ola y tecla a tecla: solo se reenvían las teclas que cambian
pub(super) async fn play_effect(
    device: &Device,
    model: &Model,
    cal: &Calibration,
    anim: &Animation,
    closing: bool,
) -> Result<usize, MirajazzError> {
    let (rows, cols) = anim_grid(model);
    let (rows_f, cols_f) = (rows as f32, cols as f32);
    let n = (rows as usize) * (cols as usize);
    let colors = palette(anim);
    let levels = model.anim_levels();
    let duration = Duration::from_millis(anim.duration_ms.clamp(MIN_MS, MAX_MS) as u64);
    let mut last: Vec<Option<[u8; 3]>> = vec![None; n];
    let mut screen = Screen::new(device, model, cal, !closing);
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
                    screen.set(i as u8, Some(c)).await?;
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
