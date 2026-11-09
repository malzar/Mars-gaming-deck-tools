//! Calibración de los iconos del MSD-ONE (protocolo 1).
//!
//! El firmware solo admite 85×85 px por tecla, pero no los pinta todos donde
//! toca: recorta un poco y los desplaza (y en la pantalla lateral, más).
//! Aquí se mueve y escala cada icono dentro de esos 85×85 para compensarlo.

use image::{DynamicImage, Rgb, RgbImage, imageops, imageops::FilterType};

use crate::settings::{Adjust, Calibration};

pub const SIZE: u32 = 85;

/// Columna de la pantalla lateral en la cuadrícula de OpenDeck (6 columnas)
pub fn is_side(position: u8) -> bool {
    position % 6 == 5
}

pub fn adjust_for(cal: &Calibration, position: u8) -> Adjust {
    if is_side(position) { cal.side } else { cal.keys }
}

/// Coloca la imagen dentro de un lienzo de 85×85 con el desplazamiento y tamaño pedidos
pub fn apply(img: DynamicImage, adj: Adjust) -> DynamicImage {
    if adj == Adjust::default() {
        return img;
    }
    let s = (SIZE * adj.size.clamp(40, 130) / 100).max(1);
    let content = img.resize_exact(s, s, FilterType::Lanczos3).to_rgb8();
    let mut canvas = RgbImage::from_pixel(SIZE, SIZE, Rgb([0, 0, 0]));
    let off = (SIZE as i64 - s as i64) / 2;
    imageops::overlay(&mut canvas, &content, off + adj.dx as i64, off + adj.dy as i64);
    DynamicImage::ImageRgb8(canvas)
}

/// Imagen de prueba: anillos de colores (3 px cada uno), flecha hacia arriba
/// y marcas blancas en las esquinas de arriba
pub fn test_pattern() -> DynamicImage {
    const RINGS: [[u8; 3]; 7] = [
        [255, 32, 32], [255, 138, 0], [255, 230, 0], [32, 208, 64],
        [0, 200, 255], [48, 80, 255], [160, 64, 255],
    ];
    let mut img = RgbImage::from_pixel(SIZE, SIZE, Rgb([0, 0, 0]));
    for (y, x, p) in img.enumerate_pixels_mut().map(|(x, y, p)| (y as i32, x as i32, p)) {
        let edge = x.min(y).min(SIZE as i32 - 1 - x).min(SIZE as i32 - 1 - y);
        let ring = (edge / 3) as usize;
        if ring < RINGS.len() {
            *p = Rgb(RINGS[ring]);
        } else if edge >= 24 {
            *p = Rgb([12, 20, 64]);
        }
        // Flecha hacia arriba
        let (cx, top) = (42, 30);
        if (top..top + 16).contains(&y) && (x - cx).abs() <= (y - top) / 2 {
            *p = Rgb([255, 255, 255]);
        }
        if (48..52).contains(&y) && (36..49).contains(&x) {
            *p = Rgb([255, 255, 255]);
        }
        // Marcas en las esquinas de arriba
        if y < 5 && (x < 5 || x >= SIZE as i32 - 5) {
            *p = Rgb([255, 255, 255]);
        }
    }
    DynamicImage::ImageRgb8(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_ajuste_no_toca_la_imagen() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(144, 144));
        assert_eq!(apply(img, Adjust::default()).width(), 144);
    }

    #[test]
    fn desplaza_y_escala() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(10, 10, Rgb([255, 255, 255])));
        let out = apply(img, Adjust { dx: 5, dy: -3, size: 50 }).to_rgb8();
        assert_eq!(out.dimensions(), (85, 85));
        // 42 px de lado, centrado en 21 + desplazamiento
        assert_eq!(out.get_pixel(26, 18).0, [255, 255, 255]);
        assert_eq!(out.get_pixel(25, 18).0, [0, 0, 0]);
        assert_eq!(out.get_pixel(26, 17).0, [0, 0, 0]);
        assert!(is_side(5) && is_side(11) && is_side(17) && !is_side(4));
    }
}

#[cfg(test)]
#[test]
#[ignore]
fn guardar_patron() {
    let p = std::env::var("MGDC_PATTERN_OUT").unwrap();
    test_pattern().resize(255, 255, FilterType::Nearest).save(p).unwrap();
}
