//! Calibración de los iconos.
//!
//! El firmware de estos decks no siempre pinta la imagen donde toca: recorta un
//! poco y la desplaza (en el MSD-ONE, sobre todo en la pantalla lateral). Aquí
//! se mueve y escala cada icono dentro de su tamaño real para compensarlo.

use image::{DynamicImage, Rgb, RgbImage, imageops, imageops::FilterType};

use crate::settings::{Adjust, Calibration};

/// Columna de la pantalla lateral del MSD-ONE en la cuadrícula de OpenDeck (6 columnas)
pub fn is_side(position: u8) -> bool {
    position % 6 == 5
}

/// Ajuste de una tecla del MSD-ONE (teclas principales o pantalla lateral)
pub fn adjust_for(cal: &Calibration, position: u8) -> Adjust {
    if is_side(position) { cal.side } else { cal.keys }
}

/// Coloca la imagen en un lienzo de `w`×`h` con el desplazamiento y tamaño pedidos.
/// El contenido se escala para encajar en el lienzo sin deformarse.
pub fn apply_sized(img: DynamicImage, adj: Adjust, (w, h): (u32, u32)) -> DynamicImage {
    if adj == Adjust::default() {
        return img;
    }
    let k = adj.size.clamp(40, 130) as f32 / 100.0;
    let fit = (w as f32 / img.width() as f32).min(h as f32 / img.height() as f32);
    let cw = ((img.width() as f32 * fit * k).round() as u32).max(1);
    let ch = ((img.height() as f32 * fit * k).round() as u32).max(1);
    let content = img.resize_exact(cw, ch, FilterType::Lanczos3).to_rgb8();
    let mut canvas = RgbImage::from_pixel(w, h, Rgb([0, 0, 0]));
    let x = (w as i64 - cw as i64) / 2 + adj.dx as i64;
    let y = (h as i64 - ch as i64) / 2 + adj.dy as i64;
    imageops::overlay(&mut canvas, &content, x, y);
    DynamicImage::ImageRgb8(canvas)
}


/// Imagen de prueba: anillos de colores (3 px cada uno en 85 px), flecha
/// hacia arriba y marcas blancas en las 4 esquinas
pub fn test_pattern_sized(w: u32, h: u32) -> DynamicImage {
    const RINGS: [[u8; 3]; 7] = [
        [255, 32, 32], [255, 138, 0], [255, 230, 0], [32, 208, 64],
        [0, 200, 255], [48, 80, 255], [160, 64, 255],
    ];
    let (wi, hi) = (w as i32, h as i32);
    let unit = (wi.min(hi) as f32 / 85.0).max(0.5);
    let u = |v: f32| (v * unit).round() as i32;
    let ring_w = u(3.0).max(1);
    let mut img = RgbImage::from_pixel(w, h, Rgb([0, 0, 0]));
    for (y, x, p) in img.enumerate_pixels_mut().map(|(x, y, p)| (y as i32, x as i32, p)) {
        let edge = x.min(y).min(wi - 1 - x).min(hi - 1 - y);
        let ring = (edge / ring_w) as usize;
        if ring < RINGS.len() {
            *p = Rgb(RINGS[ring]);
        } else if edge >= ring_w * 8 {
            *p = Rgb([12, 20, 64]);
        }
        // Flecha hacia arriba
        let (cx, top) = (wi / 2, hi / 2 - u(12.0));
        if (top..top + u(16.0)).contains(&y) && (x - cx).abs() <= (y - top) / 2 {
            *p = Rgb([255, 255, 255]);
        }
        if (hi / 2 + u(6.0)..hi / 2 + u(10.0)).contains(&y) && (x - cx).abs() <= u(6.0) {
            *p = Rgb([255, 255, 255]);
        }
        // Marcas blancas en las 4 esquinas
        let m = u(5.0);
        if (y < m || y >= hi - m) && (x < m || x >= wi - m) {
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
        assert_eq!(apply_sized(img, Adjust::default(), (85, 85)).width(), 144);
    }

    #[test]
    fn desplaza_y_escala() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(10, 10, Rgb([255, 255, 255])));
        let out = apply_sized(img, Adjust { dx: 5, dy: -3, size: 50 }, (85, 85)).to_rgb8();
        assert_eq!(out.dimensions(), (85, 85));
        // 42 px de lado, centrado en 21 + desplazamiento
        assert_eq!(out.get_pixel(26, 18).0, [255, 255, 255]);
        assert_eq!(out.get_pixel(25, 18).0, [0, 0, 0]);
        assert_eq!(out.get_pixel(26, 17).0, [0, 0, 0]);
        assert!(is_side(5) && is_side(11) && is_side(17) && !is_side(4));
    }

    #[test]
    fn rectangulo_sin_deformar() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(176, 112, Rgb([255, 255, 255])));
        let out = apply_sized(img, Adjust { dx: 0, dy: 0, size: 50 }, (176, 112)).to_rgb8();
        assert_eq!(out.dimensions(), (176, 112));
        assert_eq!(out.get_pixel(44, 28).0, [255, 255, 255]);
        assert_eq!(out.get_pixel(43, 28).0, [0, 0, 0]);
        assert_eq!(test_pattern_sized(176, 112).width(), 176);
    }
}

#[cfg(test)]
#[test]
#[ignore]
fn guardar_patron() {
    let p = std::env::var("MGDC_PATTERN_OUT").unwrap();
    test_pattern_sized(176, 112).resize(528, 336, FilterType::Nearest).save(p).unwrap();
}
