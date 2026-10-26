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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LedMode {
    Off,
    #[default]
    One,
    /// Un color por rueda
    Each,
    /// Un color que sube y baja
    Breath,
    /// Recorre los colores de una paleta
    Palette,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Leds {
    pub mode: LedMode,
    /// 4 colores "#rrggbb" (uno por rueda; en "un color" y "respirar" se usa el primero)
    pub colors: Vec<String>,
    pub palette: String,
    pub palette_colors: Vec<String>,
    pub brightness: u8,
    /// Destello al girar una rueda
    pub flash_on_turn: bool,
    /// Se apagan con la pantalla y siguen el brillo de día/noche
    pub follow_screen: bool,
}

impl Default for Leds {
    fn default() -> Self {
        Self {
            mode: LedMode::One,
            colors: vec!["#2f7de1".into(); 4],
            palette: "Arcoíris".into(),
            palette_colors: Animation::default().colors,
            brightness: 80,
            flash_on_turn: true,
            follow_screen: true,
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
    /// LED de las ruedas (familia N4). None = no se ha tocado en el panel:
    /// se respeta el config.toml de ambiso.
    pub leds: Option<Leds>,
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
            leds: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub devices: HashMap<String, DeviceSettings>,
    /// Paletas creadas por el usuario (las de serie van en el panel)
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
