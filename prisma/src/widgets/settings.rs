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
    /// Aviso de lluvia: si va a llover en los próximos `rain_minutes` (con al
    /// menos `rain_prob` % de probabilidad por horas)
    pub alert_rain: bool,
    pub rain_minutes: u32,
    pub rain_prob: u8,
    /// Avisos oficiales del organismo del país (`alert_source`: su id de la
    /// lista de la OMM; vacío = el más cercano; "none" = ninguno)
    pub alert_official: bool,
    pub alert_source: String,
    /// Avisos calculados con la previsión, y sus umbrales
    pub alert_calc: bool,
    pub heat_c: f32,
    pub cold_c: f32,
    pub gust_kmh: f32,
    pub alert_storm: bool,
    pub alert_snow: bool,
    /// Vista «Lluvia»: cuánto tiempo enseña, en minutos (de 15 a 2 días)
    pub rain_span: u32,
    /// Sonido de cada tipo de aviso ("" = ninguno)
    pub snd_rain: String,
    pub snd_heat: String,
    pub snd_cold: String,
    pub snd_wind: String,
    pub snd_storm: String,
    pub snd_snow: String,
    pub snd_other: String,
    /// Astronomía: la segunda página del Sol, "events" (eventos del día) o
    /// "light" (fases de luz)
    pub sun_mode: String,
    /// Astronomía: la estrella de la vista «star», de cuando solo había una
    /// (ahora cada estrella va en las vistas como «star:Nombre»)
    pub star: String,
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
            alert_rain: true,
            rain_minutes: 60,
            rain_prob: 50,
            alert_official: true,
            alert_source: String::new(),
            alert_calc: true,
            heat_c: 38.0,
            cold_c: -5.0,
            gust_kmh: 70.0,
            alert_storm: true,
            alert_snow: true,
            rain_span: 120,
            snd_rain: "lluvia".into(),
            snd_heat: "sarten".into(),
            snd_cold: "ventisca".into(),
            snd_wind: "viento".into(),
            snd_storm: "trueno".into(),
            snd_snow: "ventisca".into(),
            snd_other: "alarma".into(),
            sun_mode: "events".into(),
            star: String::new(),
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
        s.views = vec!["sun".into(), "nada".into(), "wind".into()];
        assert_eq!(s.views(false), vec!["sun", "wind"]);
        assert_eq!(s.views(true), vec!["sun", "wind"]);
    }
}
