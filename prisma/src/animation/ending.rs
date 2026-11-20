//! Cómo se queda el deck al terminar el cierre: el fondo del deck, el logo
//! de Prisma, una foto o la pantalla apagada

use std::{path::PathBuf, time::Duration};

use image::{DynamicImage, imageops::FilterType};
use mirajazz::{device::Device, error::MirajazzError};

use super::screen::{Layout, draw_canvas};
use crate::{family::Model, settings::Calibration};

/// Cómo se queda el deck al terminar el cierre
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ending {
    /// Teclas en negro: se ve el fondo del deck
    Background,
    /// El logo de Prisma por todo el deck
    Logo,
    /// La foto elegida en el panel
    Photo,
    /// Pantalla apagada
    Off,
}

impl Ending {
    pub fn from_kind(k: crate::settings::EndKind, model: &Model) -> Self {
        use crate::settings::EndKind as K;
        match k {
            K::Background => Ending::Background,
            K::Logo => Ending::Logo,
            K::Photo => Ending::Photo,
            // El MSD-ONE se queda como mínimo con la imagen de fondo
            K::Off if !model.supports_animations() => Ending::Background,
            K::Off => Ending::Off,
        }
    }

    pub fn from_settings(a: &crate::settings::Animations, model: &Model) -> Self {
        use crate::settings::EndKind as K;
        let k = a.ending.unwrap_or(if a.end_with_logo {
            K::Logo
        } else if a.end_with_background {
            K::Background
        } else {
            K::Off
        });
        Self::from_kind(k, model)
    }
}

/// Foto para el final del cierre, por deck
pub fn photo_path(id: &str) -> Option<PathBuf> {
    let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    Some(crate::app_dir()?.join("animations").join(format!("{safe}-cierre.png")))
}

/// Pinta la foto del cierre por todo el deck (teclas y pantallas de las
/// ruedas), recortada para llenarlo. Sin foto, false.
pub async fn show_photo(device: &Device, model: &Model, id: &str, cal: &Calibration) -> Result<bool, MirajazzError> {
    let Some(img) = photo_path(id).and_then(|p| image::open(p).ok()) else { return Ok(false) };
    let l = Layout::of(model);
    let canvas = img.resize_to_fill(l.w, l.h(), FilterType::Lanczos3);
    draw_canvas(device, model, cal, &l, &canvas).await?;
    device.flush().await?;
    Ok(true)
}

const LOGO: &[u8] = include_bytes!("../../assets/logo.png");
/// Color del fondo del logo
const LOGO_BG: [u8; 3] = [24, 29, 49];

/// El logo centrado en un lienzo de w×h con su color de fondo
pub(super) fn logo_canvas(w: u32, h: u32) -> DynamicImage {
    let mut canvas = image::RgbImage::from_pixel(w, h, image::Rgb(LOGO_BG));
    if let Ok(logo) = image::load_from_memory(LOGO) {
        let side = w.min(h);
        let logo = logo.resize(side, side, FilterType::Lanczos3).to_rgb8();
        image::imageops::overlay(&mut canvas, &logo, ((w - logo.width()) / 2) as i64, ((h - logo.height()) / 2) as i64);
    }
    DynamicImage::ImageRgb8(canvas)
}

/// Pinta el logo repartido entre las teclas y, en el MSD-PRO, también por
/// las pantallas de las ruedas (así sale más grande)
pub async fn show_logo(device: &Device, model: &Model, cal: &Calibration) -> Result<(), MirajazzError> {
    let l = Layout::of(model);
    let canvas = logo_canvas(l.w, l.h());
    draw_canvas(device, model, cal, &l, &canvas).await?;
    device.flush().await
}

/// Final del cierre: fondo del deck, logo de Prisma o deck apagado
pub async fn finish_closing(device: &Device, model: &Model, id: &str, ending: Ending, brightness: u8) -> Result<(), MirajazzError> {
    match ending {
        Ending::Background => {
            clear(device).await?;
            device.set_brightness(brightness).await
        }
        Ending::Logo => {
            let cal = crate::settings::for_device(id).await.calibration;
            show_logo(device, model, &cal).await?;
            device.set_brightness(brightness).await
        }
        Ending::Photo => {
            let cal = crate::settings::for_device(id).await.calibration;
            // Sin foto se queda el fondo del deck
            if !show_photo(device, model, id, &cal).await? {
                clear(device).await?;
            }
            device.set_brightness(brightness).await
        }
        Ending::Off => device.shutdown().await,
    }
}

/// Prueba del final del cierre desde el panel: lo enseña un momento
pub async fn preview_ending(device: &Device, model: &Model, id: &str, ending: Ending, brightness: u8) -> Result<(), MirajazzError> {
    let cal = crate::settings::for_device(id).await.calibration;
    match ending {
        Ending::Background => clear(device).await?,
        Ending::Logo => show_logo(device, model, &cal).await?,
        Ending::Photo => {
            if !show_photo(device, model, id, &cal).await? {
                clear(device).await?;
            }
        }
        Ending::Off => {
            clear(device).await?;
            device.set_brightness(0).await?;
        }
    }
    tokio::time::sleep(Duration::from_millis(2500)).await;
    device.set_brightness(brightness).await
}

/// Deja todas las teclas en negro (se ve el fondo del deck)
pub async fn clear(device: &Device) -> Result<(), MirajazzError> {
    device.clear_all_button_images().await?;
    device.flush().await
}
#[cfg(test)]
mod tests {
    #[test]
    fn logo_del_tamano_pedido() {
        let l = super::logo_canvas(560, 224);
        assert_eq!((l.width(), l.height()), (560, 224));
    }
}
