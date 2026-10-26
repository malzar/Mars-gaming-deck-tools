//! LED de las ruedas de la familia N4 (MSD-PRO): 4 LED, uno por rueda.
//!
//! El brillo se aplica escalando los colores, así los efectos (respirar,
//! paleta, destello al girar) y el seguimiento de la pantalla salen suaves.

use std::{
    collections::HashMap,
    f32::consts::PI,
    sync::LazyLock,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

use crate::{
    DEVICES,
    animation::{sample, scale},
    power,
    settings::{self, LedMode, Leds, parse_hex},
};

pub const LED_COUNT: usize = 4;
const FLASH: Duration = Duration::from_millis(220);

/// Último giro de cada rueda, para el destello
static TWISTS: LazyLock<Mutex<HashMap<String, [Option<Instant>; LED_COUNT]>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn on_twist(id: &str, encoder: u8) {
    if (encoder as usize) < LED_COUNT {
        TWISTS.lock().await.entry(id.to_string()).or_default()[encoder as usize] = Some(Instant::now());
    }
}

/// Colores de los 4 LED en el instante `t` (segundos)
pub fn frame(leds: &Leds, t: f32, screen: u8, flashes: [f32; LED_COUNT]) -> [[u8; 3]; LED_COUNT] {
    let color = |i: usize| parse_hex(leds.colors.get(i).or(leds.colors.first()).map_or("#ffffff", |s| s));
    let palette: Vec<[u8; 3]> = leds.palette_colors.iter().map(|c| parse_hex(c)).collect();

    let mut level = leds.brightness.min(100) as f32 / 100.0;
    if leds.follow_screen {
        level *= screen.min(100) as f32 / 100.0;
    }

    std::array::from_fn(|i| {
        let base = match leds.mode {
            LedMode::Off => [0, 0, 0],
            LedMode::One => color(0),
            LedMode::Each => color(i),
            LedMode::Breath => scale(color(0), 0.15 + 0.85 * ((t * 2.0 * PI / 3.0).sin() + 1.0) / 2.0),
            LedMode::Palette => sample(&palette, t * 0.15 + i as f32 / LED_COUNT as f32),
        };
        let mut c = scale(base, level);
        // Destello hacia blanco al girar la rueda
        let f = flashes[i];
        if f > 0.0 && leds.mode != LedMode::Off && (!leds.follow_screen || screen > 0) {
            let peak = (level.max(0.35) * 255.0) as f32;
            c = c.map(|v| (v as f32 + (peak - v as f32).max(0.0) * f).round() as u8);
        }
        c
    })
}

/// Mantiene los LED al día mientras el deck está conectado (no termina nunca)
pub async fn led_task(id: String) {
    let start = Instant::now();
    let mut last: Option<[[u8; 3]; LED_COUNT]> = None;
    let mut took_control = false;

    loop {
        tokio::time::sleep(Duration::from_millis(50)).await;

        let Some(leds) = settings::for_device(&id).await.leds else {
            // Sin tocar en el panel: se respeta el config.toml
            took_control = false;
            continue;
        };

        let flashes = {
            let tw = TWISTS.lock().await;
            tw.get(&id).map_or([0.0; LED_COUNT], |a| {
                a.map(|t| t.map_or(0.0, |t| 1.0 - (t.elapsed().as_secs_f32() / FLASH.as_secs_f32()).min(1.0)))
            })
        };
        let colors = frame(&leds, start.elapsed().as_secs_f32(), power::screen_level(&id).await, flashes);

        if took_control && last == Some(colors) {
            continue;
        }

        // Nunca termina por su cuenta: si algo falla, espera y lo vuelve a intentar
        let result = {
            let devices = DEVICES.read().await;
            match devices.get(&id) {
                None => Ok(()),
                Some(device) => async {
                    if !took_control {
                        device.set_led_brightness(100).await?;
                        took_control = true;
                    }
                    device.set_led_colors(&colors).await
                }
                .await,
            }
        };
        match result {
            Ok(()) => last = Some(colors),
            Err(e) => {
                log::warn!("LED de {id}: {e}");
                took_control = false;
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apagado_y_brillo() {
        let mut l = Leds::default();
        l.colors = vec!["#ff0000".into(); 4];
        l.brightness = 50;
        l.follow_screen = false;
        assert_eq!(frame(&l, 0.0, 100, [0.0; 4])[0], [128, 0, 0]);
        l.follow_screen = true;
        assert_eq!(frame(&l, 0.0, 0, [0.0; 4])[2], [0, 0, 0]);
        l.mode = LedMode::Off;
        assert_eq!(frame(&l, 0.0, 100, [1.0; 4])[1], [0, 0, 0]);
    }
}
