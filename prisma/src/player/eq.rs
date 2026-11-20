//! El ecualizador: barras simuladas o que siguen el volumen real

use super::{EQ_BARS, EqMode};

/// Forma de las barras cuando siguen el volumen: más altas en el centro
const SHAPE: [f32; EQ_BARS] = [0.55, 0.8, 1.0, 0.9, 1.0, 0.75, 0.5, 0.7, 0.95, 0.85, 0.65, 0.5];

/// Siguiente altura de las barras. `level` = volumen real (0-1) en el modo
/// "volumen real".
pub fn next_eq(prev: [f32; EQ_BARS], mode: EqMode, level: f32, seed: u64) -> [f32; EQ_BARS] {
    let mut out = [0.0; EQ_BARS];
    for i in 0..EQ_BARS {
        let noise = rand01(seed.wrapping_mul(31).wrapping_add(i as u64));
        let target = match mode {
            EqMode::Simulated => 0.2 + 0.8 * noise,
            EqMode::Level => (level * 1.6).min(1.0) * SHAPE[i] * (0.65 + 0.35 * noise),
        };
        // Sube rápido y baja despacio, como un ecualizador de verdad
        out[i] = if target > prev[i] { prev[i] + (target - prev[i]) * 0.7 } else { prev[i] + (target - prev[i]) * 0.3 };
    }
    out
}

/// Número "al azar" de 0 a 1 a partir de una semilla (sin depender de nada)
fn rand01(mut x: u64) -> f32 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^= x >> 33;
    (x % 10_000) as f32 / 10_000.0
}
