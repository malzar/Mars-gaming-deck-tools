//! Ajustes de MGDC por deck, guardados en %APPDATA%\mgdc\settings.json

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::LazyLock};
use tokio::sync::{Notify, RwLock};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleMode {
    /// Amanecer y puesta de sol calculados según la ubicación
    #[default]
    Sun,
    /// Horas fijas
    Fixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Schedule {
    pub enabled: bool,
    pub mode: ScheduleMode,
    pub city: String,
    pub lat: f64,
    pub lon: f64,
    /// Minutos que se adelanta (negativo) o retrasa (positivo) el cambio respecto al sol
    pub offset_minutes: i32,
    /// "HH:MM" para el modo de horario fijo
    pub day_start: String,
    pub night_start: String,
    pub day_brightness: u8,
    pub night_brightness: u8,
    /// Duración de la transición suave entre brillos
    pub transition_minutes: u32,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: ScheduleMode::Sun,
            city: "Madrid".into(),
            lat: 40.4168,
            lon: -3.7038,
            offset_minutes: 0,
            day_start: "07:30".into(),
            night_start: "21:00".into(),
            day_brightness: 80,
            night_brightness: 20,
            transition_minutes: 30,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AnimKind {
    #[default]
    None,
    /// Ola de color en diagonal
    Wave,
    /// Las teclas se encienden (o apagan) una a una
    Keys,
    /// Fundido de todas las teclas a la vez
    Fade,
    /// GIF propio repartido por la cuadrícula de teclas
    Gif,
    /// Tus propios iconos aparecen (o se van) tecla a tecla
    Cascade,
    /// Una sola tecla encendida que recorre el deck de la primera a la última
    Chase,
    /// Flecha que avanza de izquierda a derecha: la fila del centro va delante
    Arrow,
    /// Se enciende una fila entera, luego la siguiente y se apaga la anterior
    Rows,
    /// Teclas al azar, de una en una y sin repetir
    Random,
    /// Zigzag: una fila a la derecha, la siguiente a la izquierda
    Snake,
    /// Espiral desde el borde hacia el centro
    Spiral,
    /// Una tecla que rebota en diagonal contra los bordes
    Bounce,
    /// Dos teclas que salen de los extremos y se encuentran en el centro
    Meet,
    /// Gotas que caen por columnas al azar
    Rain,
    /// Dos teclas opuestas que giran por el borde
    Windmill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Animation {
    pub kind: AnimKind,
    pub duration_ms: u32,
    /// Nombre de la paleta elegida en el panel (solo para mostrarla)
    pub palette: String,
    /// Colores "#rrggbb": uno si es "un color", varios si es una paleta
    pub colors: Vec<String>,
    /// Cada tecla de un color al azar (en vez de los colores de arriba)
    pub random_colors: bool,
    /// Pasos de estela (1-3): cuántos pasos tarda en apagarse una tecla
    pub trail: u8,
    /// Fijo: sin estela, las teclas se quedan encendidas hasta el final
    pub fixed: bool,
    /// En modo fijo, cómo se apagan al final
    pub fixed_off: FixedOff,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FixedOff {
    /// Todas a la vez
    #[default]
    All,
    /// Con el mismo recorrido que al encender
    Same,
}

impl Animation {
    fn new(kind: AnimKind) -> Self {
        Self {
            kind,
            duration_ms: 1500,
            palette: "Arcoíris".into(),
            colors: vec![
                "#ff0000".into(), "#ff8a00".into(), "#ffe600".into(), "#00d84a".into(),
                "#00b3ff".into(), "#5a3dff".into(), "#d400ff".into(),
            ],
            random_colors: false,
            trail: 3,
            fixed: false,
            fixed_off: FixedOff::All,
        }
    }
}

impl Default for Animation {
    fn default() -> Self {
        Self::new(AnimKind::None)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Animations {
    pub startup: Animation,
    pub shutdown: Animation,
    /// Al terminar el cierre: true = se ve el fondo del deck, false = pantalla apagada
    pub end_with_background: bool,
}

impl Default for Animations {
    fn default() -> Self {
        Self {
            startup: Animation::new(AnimKind::None),
            shutdown: Animation::new(AnimKind::None),
            end_with_background: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceSettings {
    /// Brillo cuando el horario de día/noche está desactivado
    pub brightness: u8,
    /// Si es true, la pantalla nunca se apaga sola
    pub never_sleep: bool,
    /// Minutos sin usar el deck antes de apagar la pantalla
    pub sleep_minutes: u32,
    /// La pulsación que despierta la pantalla solo la enciende (no ejecuta la acción)
    pub swallow_wake_press: bool,
    pub schedule: Schedule,
    pub animations: Animations,
    /// Corrección de posición y tamaño de los iconos
    pub calibration: Calibration,
}

/// Desplazamiento (px) y tamaño (%) del icono dentro de los 85×85 px de la tecla
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Adjust {
    pub dx: i32,
    pub dy: i32,
    pub size: u32,
}

impl Default for Adjust {
    fn default() -> Self {
        Self { dx: 0, dy: 0, size: 100 }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Calibration {
    /// Las 15 teclas
    pub keys: Adjust,
    /// Los 3 iconos de la pantalla lateral (MSD-ONE)
    pub side: Adjust,
    /// La pantalla de las ruedas (MSD-PRO)
    pub strip: Adjust,
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            brightness: 70,
            never_sleep: true,
            sleep_minutes: 15,
            swallow_wake_press: true,
            schedule: Schedule::default(),
            animations: Animations::default(),
            calibration: Calibration::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub devices: HashMap<String, DeviceSettings>,
    /// Paletas creadas por el usuario para las animaciones (las de serie van en el panel)
    pub palettes: Vec<Palette>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    pub colors: Vec<String>,
}

pub async fn set_palettes(palettes: Vec<Palette>) {
    SETTINGS.write().await.palettes = palettes;
    save().await;
}

/// "#rrggbb" a [r, g, b] (negro si no es válido)
pub fn parse_hex(s: &str) -> [u8; 3] {
    let h = s.trim().trim_start_matches('#');
    match u32::from_str_radix(h, 16) {
        Ok(v) if h.len() == 6 => [(v >> 16) as u8, (v >> 8) as u8, v as u8],
        _ => [0, 0, 0],
    }
}

pub static SETTINGS: LazyLock<RwLock<AppSettings>> = LazyLock::new(|| RwLock::new(load()));

/// Avisa al planificador de que algo ha cambiado (ajustes, pulsación que despierta…)
pub static CHANGED: LazyLock<Notify> = LazyLock::new(Notify::new);

fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("mgdc").join("settings.json"))
}

fn load() -> AppSettings {
    let Some(p) = path() else {
        return AppSettings::default();
    };
    match std::fs::read_to_string(&p) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            log::warn!("settings.json no válido ({e}), uso valores por defecto");
            AppSettings::default()
        }),
        Err(_) => AppSettings::default(),
    }
}

pub async fn save() {
    let snapshot = SETTINGS.read().await.clone();
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(&snapshot) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&p, text) {
                log::warn!("No se pudo guardar {}: {e}", p.display());
            }
        }
        Err(e) => log::warn!("No se pudieron serializar los ajustes: {e}"),
    }
}

pub async fn for_device(id: &str) -> DeviceSettings {
    SETTINGS
        .read()
        .await
        .devices
        .get(id)
        .cloned()
        .unwrap_or_default()
}

pub async fn set_for_device(id: &str, settings: DeviceSettings) {
    SETTINGS
        .write()
        .await
        .devices
        .insert(id.to_string(), settings);
    save().await;
    CHANGED.notify_one();
}
