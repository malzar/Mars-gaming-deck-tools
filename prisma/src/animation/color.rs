//! Colores de las animaciones: la paleta, el brillo y los colores al azar

use image::{DynamicImage, Rgb, RgbImage};

use crate::settings::{Animation, parse_hex};

/// Color de una paleta en la posición `t` (0..1, cíclica), interpolando
pub(super) fn sample(colors: &[[u8; 3]], t: f32) -> [u8; 3] {
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

pub(super) fn scale(c: [u8; 3], a: f32) -> [u8; 3] {
    let a = a.clamp(0.0, 1.0);
    c.map(|v| (v as f32 * a).round() as u8)
}

pub(super) fn solid(c: [u8; 3]) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_pixel(8, 8, Rgb(c)))
}

pub(super) fn palette(anim: &Animation) -> Vec<[u8; 3]> {
    let mut colors: Vec<[u8; 3]> = anim.colors.iter().map(|c| parse_hex(c)).collect();
    if colors.is_empty() {
        colors.push([255, 255, 255]);
    }
    colors
}

/// Generador pseudoaleatorio sencillo (xorshift), sin dependencias
pub(super) struct Rng(u64);

impl Rng {
    pub(super) fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Rng(seed | 1)
    }
    pub(super) fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub(super) fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Color vivo al azar (tono aleatorio, saturación y brillo al máximo)
pub(super) fn random_color(rng: &mut Rng) -> [u8; 3] {
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
