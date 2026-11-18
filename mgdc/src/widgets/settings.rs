//! Ajustes de cada tecla o rueda de hora, fecha y tiempo (los guarda su
//! ventana de ajustes)

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct WSettings {
    /// Hora en tecla: "digital" o "analog"
    pub style: String,
    /// Hora en tecla: "full", "hours" o "minutes" (para montar una columna en
    /// los laterales del MSD-ONE)
    pub part: String,
    /// Hora digital en tecla: el día debajo
    pub show_date: bool,
    /// Hora en la rueda: los segundos
    pub seconds: bool,
    /// Tiempo: sitio (vacío = el del horario de día/noche del panel)
    pub city: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    /// Tiempo: vistas, en orden (ver `weather::VIEWS`)
    pub views: Vec<String>,
    /// Pasar solo a la siguiente vista cada X segundos (0 = no)
    pub rotate_secs: u32,
    /// Tiempo, al pulsar y al pulsar dos veces: "next", "web" o "none"
    pub click: String,
    pub double: String,
    /// Hora en tecla, al pulsar y al pulsar dos veces: "style" (agujas ↔
    /// digital) o "none"
    pub clock_click: String,
    pub clock_double: String,
}

impl Default for WSettings {
    fn default() -> Self {
        Self {
            style: "digital".into(),
            part: "full".into(),
            show_date: true,
            seconds: true,
            city: String::new(),
            lat: None,
            lon: None,
            views: vec![],
            rotate_secs: 0,
            click: "next".into(),
            double: "web".into(),
            clock_click: "style".into(),
            clock_double: "none".into(),
        }
    }
}

impl WSettings {
    pub fn from_value(v: &serde_json::Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }

    /// Vistas del tiempo elegidas que valen para tecla o rueda (al menos una)
    pub fn views(&self, dial: bool) -> Vec<&'static str> {
        let v: Vec<&'static str> = self.views.iter().filter_map(|x| super::weather::view(x, dial).map(|v| v.id)).collect();
        if v.is_empty() { vec!["now"] } else { v }
    }

    /// Si el doble click hace algo (si no, el click va al momento)
    pub fn has_double(&self) -> bool {
        self.double != "none"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vistas() {
        let mut s = WSettings::default();
        assert_eq!(s.views(false), vec!["now"]);
        s.views = vec!["hours".into(), "forecast".into(), "today".into()];
        assert_eq!(s.views(false), vec!["forecast", "today"]);
        assert_eq!(s.views(true), vec!["hours", "forecast"]);
        s.views = vec!["moon".into(), "nada".into(), "wind".into()];
        assert_eq!(s.views(false), vec!["moon", "wind"]);
        assert_eq!(s.views(true), vec!["moon", "wind"]);
    }
}
