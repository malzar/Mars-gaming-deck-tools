//! Ajustes de cada acción de audio (los guarda su ventana de ajustes)

use serde::{Deserialize, Serialize};

use super::{Kind, state::INSTANCES};

/// Ajustes de cada acción (los guarda la ventana de ajustes de OpenDeck)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ActSettings {
    /// "general" o el programa ("spotify.exe")
    pub target: String,
    /// Puntos de volumen por pulsación o por clic de rueda
    pub step: u8,
    /// Mostrar lo que suena (Reproducir/Pausa y la rueda del volumen general)
    pub show_title: bool,
    /// Reproducir/Pausa: barra de abajo mientras suena (ver player.rs)
    pub bar: String,
    pub bar_color: String,
    pub eq_mode: String,
    /// Rueda: "bar" (barra), "eq" (ecualizador en lugar de la barra) o "eqbg" (de fondo)
    pub dial_style: String,
    /// Rueda de música: icono del programa que suena: "none", "state" (junto
    /// al ▶), "background" (de fondo) o "title" (junto al título)
    pub app_icon: String,
    /// Y a qué lado: "left" o "right"
    pub app_icon_side: String,
    /// La tecla está en un MSD-ONE (teclas pequeñas): no viene de los ajustes
    #[serde(skip)]
    pub small: bool,
}

impl Default for ActSettings {
    fn default() -> Self {
        Self {
            target: "general".into(),
            step: 2,
            show_title: true,
            bar: "color".into(),
            bar_color: "#2ecc71".into(),
            eq_mode: "level".into(),
            dial_style: "bar".into(),
            app_icon: "state".into(),
            app_icon_side: "left".into(),
            small: false,
        }
    }
}

impl ActSettings {
    pub(super) fn from_value(v: &serde_json::Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }

    pub(super) fn look(&self) -> crate::player::Look<'_> {
        crate::player::Look { bar: crate::player::Bar::from_id(&self.bar), color: &self.bar_color, small: self.small }
    }

    /// Reproducir/Pausa con el diseño de lo que suena (lo pinta `player_task`)
    pub(super) fn shows_player(&self, kind: Kind) -> bool {
        kind == Kind::PlayPause && self.show_title
    }
}

/// Ajustes de un evento, sabiendo ya si la tecla está en un MSD-ONE
pub(super) async fn settings_of(context: &str, v: &serde_json::Value) -> ActSettings {
    let mut s = ActSettings::from_value(v);
    s.small = INSTANCES.lock().await.get(context).is_some_and(|i| i.settings.small);
    s
}

/// Si el deck es un MSD-ONE (familia AKP153)
pub(super) async fn is_small(device: &str) -> bool {
    match crate::DEVICES.read().await.get(device) {
        Some(d) => matches!(crate::family::Model::from_vid_pid(d.vid, d.pid), Some(crate::family::Model::Akp153(_))),
        None => false,
    }
}
