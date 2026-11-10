//! Familias de dispositivos que maneja MGDC.
//!
//! - `akp153`: 15 teclas + 3 iconos laterales, sin ruedas (MSD-ONE).
//! - `akp05`: 10 teclas, 4 ruedas y tira táctil, tipo Stream Deck+ (MSD-PRO).

use std::sync::LazyLock;

use mirajazz::{
    device::{Device, DeviceQuery},
    error::MirajazzError,
    types::{DeviceInput, HidDeviceInfo},
};
use openaction::SetImageEvent;

pub mod akp05;
pub mod akp153;

/// Prefijo de los ids de dispositivo. Debe coincidir con `DeviceNamespace` del
/// manifest.json. Se usa el del plugin AKP153 ("99") para que los perfiles del
/// MSD-ONE sigan funcionando sin tocar nada.
pub const DEVICE_NAMESPACE: &str = "99";

/// Todas las combinaciones VID/PID que buscamos (las dos familias)
pub static QUERIES: LazyLock<Vec<DeviceQuery>> = LazyLock::new(|| {
    let mut queries: Vec<DeviceQuery> = akp153::QUERIES.to_vec();
    queries.extend_from_slice(akp05::QUERIES);
    queries
});

/// Modelo concreto, con la familia a la que pertenece
#[derive(Debug, Clone)]
pub enum Model {
    Akp153(akp153::Kind),
    Akp05(akp05::Kind),
}

/// Distribución física que se anuncia a OpenDeck
pub struct Layout {
    pub rows: u8,
    pub cols: u8,
    pub keys: usize,
    pub encoders: usize,
    pub device_type: u8,
}

impl Model {
    pub fn from_vid_pid(vid: u16, pid: u16) -> Option<Self> {
        akp153::Kind::from_vid_pid(vid, pid)
            .map(Model::Akp153)
            .or_else(|| akp05::Kind::from_vid_pid(vid, pid).map(Model::Akp05))
    }

    pub fn human_name(&self) -> String {
        match self {
            Model::Akp153(k) => k.human_name(),
            Model::Akp05(k) => k.human_name(),
        }
    }

    pub fn protocol_version(&self) -> usize {
        match self {
            Model::Akp153(k) => k.protocol_version(),
            Model::Akp05(k) => k.protocol_version(),
        }
    }

    pub fn layout(&self) -> Layout {
        match self {
            Model::Akp153(_) => Layout {
                rows: akp153::ROW_COUNT as u8,
                cols: akp153::COL_COUNT as u8,
                keys: akp153::KEY_COUNT,
                encoders: akp153::ENCODER_COUNT,
                device_type: 0,
            },
            Model::Akp05(_) => Layout {
                rows: akp05::ROW_COUNT as u8,
                cols: akp05::COL_COUNT as u8,
                keys: akp05::KEY_COUNT,
                encoders: akp05::ENCODER_COUNT,
                device_type: akp05::DEVICE_TYPE,
            },
        }
    }

    /// Teclas con pantalla como cuadrícula (filas, columnas) en posiciones de OpenDeck
    pub fn grid(&self) -> (u8, u8) {
        match self {
            Model::Akp153(_) => (akp153::ROW_COUNT as u8, akp153::COL_COUNT as u8),
            Model::Akp05(_) => (akp05::ROW_COUNT as u8, akp05::COL_COUNT as u8),
        }
    }

    /// El MSD-ONE (protocolo 1) no aguanta las animaciones: se atasca y se
    /// desconecta. En ese deck al cerrar solo se deja la imagen de fondo.
    pub fn supports_animations(&self) -> bool {
        !matches!(self, Model::Akp153(k) if k.protocol_version() == 1)
    }

    /// Tipos de animación que admite este deck. El MSD-ONE solo aguanta la
    /// cascada de iconos (una tecla cada vez, a su ritmo).
    pub fn anim_kinds(&self) -> &'static [&'static str] {
        if self.supports_animations() {
            &["none", "cascade", "wave", "keys", "fade", "gif"]
        } else {
            &["none", "cascade"]
        }
    }

    pub fn allows_animation(&self, kind: &crate::settings::AnimKind) -> bool {
        use crate::settings::AnimKind;
        matches!(kind, AnimKind::None | AnimKind::Cascade) || self.supports_animations()
    }

    /// Borra una tecla (o un trozo de la pantalla de las ruedas) sin enviarlo todavía
    pub async fn clear_key(&self, device: &Device, position: u8, encoder: bool) -> Result<(), MirajazzError> {
        match self {
            Model::Akp153(_) => device.clear_button_image(akp153::opendeck_to_device(position)).await,
            Model::Akp05(_) => device.clear_button_image(akp05::map_position(position, encoder)?).await,
        }
    }

    /// Ritmo de las animaciones: (pausa mínima tras cada envío, ms extra por
    /// tecla enviada). El MSD-ONE (protocolo 1) se atasca y llega a
    /// desconectarse si se le mandan tandas de teclas demasiado seguidas.
    pub fn anim_pacing(&self) -> (u64, u64) {
        match self {
            Model::Akp153(k) if k.protocol_version() == 1 => (80, 16),
            Model::Akp153(_) => (50, 6),
            Model::Akp05(_) => (40, 4),
        }
    }

    /// Niveles de intensidad de la ola: menos niveles = menos teclas que cambian
    pub fn anim_levels(&self) -> f32 {
        match self {
            Model::Akp153(k) if k.protocol_version() == 1 => 3.0,
            _ => 6.0,
        }
    }

    /// Lado en píxeles de una tecla (para el panel)
    pub fn key_size(&self) -> u32 {
        match self {
            Model::Akp153(k) if k.protocol_version() == 1 => 85,
            Model::Akp153(_) => 95,
            Model::Akp05(_) => 112,
        }
    }

    /// Zonas de calibración: (nombre, tamaño de la imagen)
    pub fn calibration_zones(&self) -> Vec<(&'static str, (u32, u32))> {
        match self {
            Model::Akp153(k) if k.protocol_version() == 1 => vec![("keys", (85, 85)), ("side", (85, 85))],
            Model::Akp153(_) => vec![("keys", (95, 95)), ("side", (82, 82))],
            Model::Akp05(k) => {
                let t = k.touch_image_format().size;
                vec![("keys", (self.key_size(), self.key_size())), ("strip", (t.0 as u32, t.1 as u32))]
            }
        }
    }

    /// Pinta la imagen de prueba de calibración en una zona (sin enviarla)
    pub async fn draw_calibration(
        &self,
        device: &Device,
        zone: &str,
        cal: &crate::settings::Calibration,
    ) -> Result<usize, MirajazzError> {
        use crate::calibration::{apply_sized, is_side, test_pattern_sized};
        let mut sent = 0;
        match self {
            Model::Akp153(_) => {
                let (rows, cols) = self.grid();
                for pos in 0..rows * cols {
                    let side = is_side(pos);
                    if (zone == "keys" && side) || (zone == "side" && !side) {
                        continue;
                    }
                    let size = if side { (82, 82) } else { (95, 95) };
                    let size = if self.protocol_version() == 1 { (85, 85) } else { size };
                    let adj = if side { cal.side } else { cal.keys };
                    let img = apply_sized(test_pattern_sized(size.0, size.1), adj, size);
                    self.draw_key(device, pos, img).await?;
                    sent += 1;
                }
            }
            Model::Akp05(k) => {
                if zone != "strip" {
                    let s = self.key_size();
                    let img = apply_sized(test_pattern_sized(s, s), cal.keys, (s, s));
                    for pos in 0..akp05::KEY_COUNT as u8 {
                        self.draw_key(device, pos, img.clone()).await?;
                        sent += 1;
                    }
                }
                if zone != "keys" {
                    let f = k.touch_image_format();
                    let size = (f.size.0 as u32, f.size.1 as u32);
                    let img = apply_sized(test_pattern_sized(size.0, size.1), cal.strip, size);
                    for enc in 0..akp05::ENCODER_COUNT as u8 {
                        device
                            .set_button_image(akp05::map_position(enc, true)?, f.clone(), img.clone())
                            .await?;
                        sent += 1;
                    }
                }
            }
        }
        Ok(sent)
    }

    /// Pinta una tecla sin enviarla todavía (hay que llamar a `device.flush()`)
    pub async fn draw_key(
        &self,
        device: &Device,
        position: u8,
        image: image::DynamicImage,
    ) -> Result<(), MirajazzError> {
        match self {
            Model::Akp153(k) => {
                device
                    .set_button_image(
                        akp153::opendeck_to_device(position),
                        akp153::get_image_format_for_key(k, position),
                        image,
                    )
                    .await
            }
            Model::Akp05(k) => {
                device
                    .set_button_image(akp05::map_position(position, false)?, k.image_format(), image)
                    .await
            }
        }
    }

    /// Descripción de los controles para el panel
    pub fn controls(&self) -> &'static str {
        match self {
            Model::Akp153(_) => "15 teclas + 3 iconos laterales",
            Model::Akp05(_) => "10 teclas · 4 ruedas · tira táctil",
        }
    }

    /// Traductor de las entradas en bruto del deck a pulsaciones
    pub fn input_processor(&self) -> fn(u8, u8) -> Result<DeviceInput, MirajazzError> {
        match self {
            Model::Akp153(_) => akp153::process_input,
            Model::Akp05(_) => akp05::process_input,
        }
    }

    pub fn supports_both_encoder_states(&self) -> Option<bool> {
        match self {
            Model::Akp153(_) => None, // se deja el valor por defecto de mirajazz
            Model::Akp05(k) => Some(k.supports_both_encoder_states()),
        }
    }

    /// La familia N4 necesita un "keep alive" periódico para no desconectarse
    pub fn needs_keepalive(&self) -> bool {
        matches!(self, Model::Akp05(_))
    }

    /// La familia AKP153 agrupa las imágenes y las envía juntas (más fluido)
    pub fn batches_flush(&self) -> bool {
        matches!(self, Model::Akp153(_))
    }

    /// Id estable del dispositivo para OpenDeck (de él dependen los perfiles)
    pub fn device_id(&self, dev: &HidDeviceInfo) -> Option<String> {
        match self {
            // Los AKP153 de protocolo 1 comparten todos el mismo serial y Windows
            // lo informa mal: se usa el fijo + un sufijo por modelo (como el original)
            Model::Akp153(k) if k.protocol_version() == 1 => Some(format!(
                "{}-355499441494-{}",
                DEVICE_NAMESPACE,
                k.id_suffix()
            )),
            _ => Some(format!(
                "{}-{}",
                DEVICE_NAMESPACE,
                dev.serial_number.clone()?
            )),
        }
    }
}

/// Pinta una imagen en el dispositivo según su familia
pub async fn set_image(device: &Device, id: &str, evt: SetImageEvent) -> Result<(), MirajazzError> {
    match Model::from_vid_pid(device.vid, device.pid) {
        Some(Model::Akp153(_)) => akp153::set_image(device, id, evt).await,
        Some(Model::Akp05(_)) => akp05::set_image(device, id, evt).await,
        None => Ok(()),
    }
}

#[derive(Debug, Clone)]
pub struct CandidateDevice {
    pub id: String,
    pub dev: HidDeviceInfo,
    pub model: Model,
}
