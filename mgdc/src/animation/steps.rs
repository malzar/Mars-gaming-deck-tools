//! Animaciones por pasos (lineal, flecha, serpiente…): reproduce los pasos con
//! su estela y solo envía las teclas que cambian

use std::time::{Duration, Instant};

use mirajazz::{device::Device, error::MirajazzError};

use super::{
    MAX_MS, MIN_MS,
    color::{Rng, palette, random_color, sample, scale},
    patterns::{Step, mirror},
    screen::Screen,
    trail::{Level, fixed_frames, trail_frames, trail_levels},
};
use crate::{
    family::Model,
    settings::{Animation, Calibration},
};

/// Reproduce una lista de pasos con estela. En cada paso solo se envían las
/// teclas que cambian (se encienden, bajan al 50 % o se apagan), y se espera
/// lo que necesita el deck según cuántas han cambiado.
pub(super) async fn play_steps(
    device: &Device,
    model: &Model,
    cal: &Calibration,
    anim: &Animation,
    steps: Vec<Step>,
    dark: bool,
) -> Result<usize, MirajazzError> {
    // Invertir: de abajo arriba y de derecha a izquierda
    let steps = if anim.reverse { mirror(model, steps) } else { steps };
    let colors = palette(anim);
    let mut rng = Rng::new();
    let (base, per_key) = model.anim_pacing();
    let levels = trail_levels(anim.trail);
    let frames = if anim.fixed {
        fixed_frames(&steps)
    } else {
        trail_frames(&steps, levels.len())
    };
    let n = frames.len().max(1);
    let even = Duration::from_millis(anim.duration_ms.clamp(MIN_MS, MAX_MS) as u64) / n as u32;
    // Color de cada tecla encendida, para que su estela sea del mismo color
    let mut color_of: std::collections::HashMap<u8, [u8; 3]> = std::collections::HashMap::new();
    let mut slowest = 0;

    let max_send = model.max_changes_per_send();
    let mut screen = Screen::new(device, model, cal, dark);
    for (i, frame) in frames.iter().enumerate() {
        // Si un paso cambia más teclas de las que aguanta el deck, se reparte en varios envíos
        let chunks: Vec<&[(u8, Level)]> = frame.chunks(max_send.max(1)).collect();
        let share = even / chunks.len() as u32;
        for changes in chunks {
            let t = Instant::now();
            for &(k, level) in changes {
                match level {
                    None => {
                        screen.set(k, None).await?;
                        color_of.remove(&k);
                    }
                    Some(0) => {
                        let c = if anim.random_colors {
                            random_color(&mut rng)
                        } else {
                            sample(&colors, i as f32 / steps.len().max(1) as f32)
                        };
                        color_of.insert(k, c);
                        screen.set(k, Some(c)).await?;
                    }
                    Some(t) => {
                        let c = scale(*color_of.get(&k).unwrap_or(&[255, 255, 255]), levels[t]);
                        screen.set(k, Some(c)).await?;
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
    log::info!("Animación {:?}: {n} pasos ({}), hasta {slowest} ms por paso", anim.kind, if anim.fixed { "fija, al final se apaga todo de golpe".to_string() } else { format!("estela de {}", levels.len() - 1) });
    Ok(n)
}
