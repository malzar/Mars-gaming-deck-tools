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
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            brightness: 70,
            never_sleep: true,
            sleep_minutes: 15,
            swallow_wake_press: true,
            schedule: Schedule::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub devices: HashMap<String, DeviceSettings>,
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
