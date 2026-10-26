use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use crate::led_config::LedConfig;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// `None` — no vibration command sent; `Some(true|false)` — enable/disable
    /// on connect. Only applied on devices whose firmware supports it.
    pub vibration: Option<bool>,
    pub leds: LedConfig,
}

/// Loads `config.toml`, then applies `leds.toml` on top of it (if present) as
/// an override for just the `[leds]` section.
pub fn load() -> Config {
    let mut config = config_path("config.toml")
        .and_then(read_toml::<Config>)
        .unwrap_or_default();

    if let Some(leds) = config_path("leds.toml").and_then(read_toml::<LedConfig>) {
        config.leds = leds;
    }

    config.leds = config.leds.resolved_colors();
    config
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Option<T> {
    let contents = fs::read_to_string(&path).ok()?;
    match toml::from_str::<T>(&contents) {
        Ok(v) => Some(v),
        Err(e) => {
            log::warn!("Failed to parse {}: {e}", path.display());
            None
        }
    }
}

/// Busca primero en la carpeta de MGDC y, si no existe el archivo, en la del
/// plugin original de ambiso (opendeck-akp05) para no perder la configuración.
fn config_path(name: &str) -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    let mgdc = base.join("mgdc").join(name);
    if mgdc.exists() {
        return Some(mgdc);
    }
    Some(base.join("opendeck-akp05").join(name))
}
