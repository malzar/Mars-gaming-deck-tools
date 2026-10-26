//! Fondo del deck (la imagen que se ve detrás de las teclas y al arrancar).
//!
//! Protocolo sacado del SDK oficial de Mirabox (StreamDock-Device-SDK):
//! cabecera `CRT\0\0LOG` + tamaño en 4 bytes big-endian + 0x01, y a continuación
//! los datos troceados en paquetes del tamaño de informe del deck.
//!
//! - Protocolo 1 (MSD-ONE, AKP153, HSV293S): 854×480 en bruto, BGR, por columnas
//!   y con espejo horizontal (como `StreamDock293s.py`).
//! - Protocolo 3, familia AKP153 (293S V3): JPEG 480×854 girado 90° a la izquierda.
//! - Familia N4 / AKP05 (MSD-PRO): JPEG 800×480 girado 180° y con espejo
//!   horizontal (como `StreamDockN4.py`).
//!
//! Por si algún clon lo pinta del revés, el panel permite añadir una corrección.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::LazyLock,
    time::{Duration, Instant},
};

use image::{DynamicImage, codecs::jpeg::JpegEncoder, imageops::FilterType};
use mirajazz::{device::Device, error::MirajazzError};
use openaction::OUTBOUND_EVENT_MANAGER;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::{DEVICES, family::Model};

/// Mientras se sube un fondo, el deck puede contestar con paquetes que no son
/// pulsaciones: se ignoran las entradas de ese deck durante un rato.
static MUTED_UNTIL: LazyLock<RwLock<HashMap<String, Instant>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn muted(id: &str) -> bool {
    MUTED_UNTIL
        .read()
        .await
        .get(id)
        .is_some_and(|t| Instant::now() < *t)
}

async fn mute(id: &str, d: Duration) {
    MUTED_UNTIL
        .write()
        .await
        .insert(id.to_string(), Instant::now() + d);
}

/// Corrección de orientación elegida en el panel
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Fix {
    #[default]
    None,
    Mirror,
    Flip,
    Rotate,
}

/// Tamaño de la pantalla tal y como la ve el usuario (ancho × alto)
pub fn screen_size(model: &Model) -> (u32, u32) {
    match model {
        Model::Akp153(_) => (854, 480),
        Model::Akp05(_) => (800, 480),
    }
}

fn packet_size(model: &Model) -> usize {
    if model.protocol_version() >= 2 { 1024 } else { 512 }
}

fn jpeg(img: &DynamicImage) -> Result<Vec<u8>, image::ImageError> {
    let rgb = img.to_rgb8();
    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, 92).encode(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(buf)
}

/// Convierte la imagen del usuario al formato que espera el deck
pub fn encode(model: &Model, img: &DynamicImage, fix: Fix) -> Result<Vec<u8>, String> {
    let (w, h) = screen_size(model);
    let img = img.resize_to_fill(w, h, FilterType::Lanczos3);
    let img = match fix {
        Fix::None => img,
        Fix::Mirror => img.fliph(),
        Fix::Flip => img.flipv(),
        Fix::Rotate => img.rotate180(),
    };

    match model {
        Model::Akp153(_) if model.protocol_version() == 1 => {
            let img = img.fliph().to_rgb8();
            let mut out = Vec::with_capacity((w * h * 3) as usize);
            for x in 0..w {
                for y in 0..h {
                    let [r, g, b] = img.get_pixel(x, y).0;
                    out.extend_from_slice(&[b, g, r]);
                }
            }
            Ok(out)
        }
        Model::Akp153(_) => jpeg(&img.rotate270()).map_err(|e| e.to_string()),
        Model::Akp05(_) => jpeg(&img.rotate180().fliph()).map_err(|e| e.to_string()),
    }
}

async fn send(device: &Device, model: &Model, data: &[u8]) -> Result<(), MirajazzError> {
    let len = data.len() as u32;
    let mut header = vec![0x00, 0x43, 0x52, 0x54, 0x00, 0x00, 0x4c, 0x4f, 0x47];
    header.extend_from_slice(&len.to_be_bytes());
    header.push(0x01);
    device.write_extended_data(&mut header).await?;

    let size = packet_size(model);
    let mut buf = Vec::with_capacity(size + 1);
    for chunk in data.chunks(size) {
        buf.clear();
        buf.push(0x00);
        buf.extend_from_slice(chunk);
        buf.resize(size + 1, 0);
        device.write_data(&buf).await?;
    }
    Ok(())
}

/// Sube el fondo al deck y vuelve a pintar las teclas
pub async fn upload(id: &str, img: &DynamicImage, fix: Fix) -> Result<(), String> {
    let started = Instant::now();

    {
        // Bloqueo exclusivo: nada más escribe en el deck mientras se sube
        let devices = DEVICES.write().await;
        let device = devices.get(id).ok_or("El deck no está conectado")?;
        let model = Model::from_vid_pid(device.vid, device.pid).ok_or("Modelo desconocido")?;
        let data = encode(&model, img, fix)?;
        log::info!(
            "Subiendo fondo a {id}: {} bytes ({:?}, corrección {:?})",
            data.len(),
            model,
            fix
        );

        mute(id, Duration::from_secs(30)).await;
        let res = send(device, &model, &data).await;
        // El deck tarda un poco en guardar la imagen
        tokio::time::sleep(Duration::from_millis(800)).await;
        res.map_err(|e| format!("Error al enviar el fondo: {e}"))?;

        device
            .clear_all_button_images()
            .await
            .map_err(|e| e.to_string())?;
        device.flush().await.map_err(|e| e.to_string())?;
    }

    mute(id, Duration::from_millis(500)).await;
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let _ = outbound.rerender_images(id.to_string()).await;
    }

    log::info!("Fondo subido a {id} en {:?}", started.elapsed());
    Ok(())
}

/// Copia de la última imagen subida, para enseñarla en el panel
pub fn saved_path(id: &str) -> Option<PathBuf> {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    Some(dirs::config_dir()?.join("mgdc").join("backgrounds").join(format!("{safe}.png")))
}

pub fn save_copy(id: &str, img: &DynamicImage) {
    let Some(path) = saved_path(id) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = img.save_with_format(&path, image::ImageFormat::Png) {
        log::warn!("No se pudo guardar la copia del fondo: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::{akp05, akp153};
    use image::{Rgb, RgbImage};

    #[test]
    fn msd_one_es_bgr_por_columnas_con_espejo() {
        // Píxel rojo arriba a la izquierda
        let mut img = RgbImage::from_pixel(854, 480, Rgb([0, 0, 0]));
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        let model = Model::Akp153(akp153::Kind::MSDONE);
        let data = encode(&model, &DynamicImage::ImageRgb8(img), Fix::None).unwrap();
        assert_eq!(data.len(), 854 * 480 * 3); // 0x12C3C0, como en el SDK
        // Con el espejo, la columna 0 del usuario es la última del deck
        let i = (853 * 480) * 3;
        assert_eq!(&data[i..i + 3], &[0, 0, 255]);
    }

    #[test]
    fn msd_pro_es_jpeg_800x480() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(1920, 1080));
        let model = Model::Akp05(akp05::Kind::MsdPro);
        let data = encode(&model, &img, Fix::None).unwrap();
        let back = image::load_from_memory(&data).unwrap();
        assert_eq!((back.width(), back.height()), (800, 480));
    }
}
