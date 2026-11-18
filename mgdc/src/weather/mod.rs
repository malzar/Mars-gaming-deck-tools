//! El tiempo: datos de Open-Meteo (gratis, sin cuenta ni clave) y sus iconos.
//!
//! Se pide una vez cada 15 minutos por sitio y se guarda en memoria; todas las
//!
//! - `open_meteo`: las consultas a Open-Meteo y cómo se leen
//! - `cache`: lo que ya se sabe de cada sitio y cuándo volver a preguntar
//! - `places`: buscar localidades por nombre
//! - `levels`: niveles de UV, aire y polen y la dirección del viento
//! - `icons`: los iconos del tiempo

mod cache;
mod icons;
mod levels;
mod open_meteo;
mod places;

pub use cache::get;
pub use icons::icon;
pub use levels::{aqi_level, compass, pollen_level, uv_level, wind_word};
pub use open_meteo::web_url;
#[cfg(test)]
pub use open_meteo::{parse, parse_air};
pub use places::search_places;

#[derive(Debug, Clone, Default)]
pub struct Day {
    /// "2026-11-16"
    pub date: String,
    pub code: u8,
    pub max: f32,
    pub min: f32,
    /// Probabilidad de lluvia (%)
    pub rain: u8,
    /// Índice UV máximo del día
    pub uv_max: f32,
    /// "07:52" y "18:05" (hora local del sitio)
    pub sunrise: String,
    pub sunset: String,
}

#[derive(Debug, Clone, Default)]
pub struct Hour {
    /// "2026-11-16T19:00"
    pub time: String,
    pub temp: f32,
    pub code: u8,
    /// Para el aviso de lluvia
    #[allow(dead_code)]
    pub rain: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Weather {
    pub temp: f32,
    pub code: u8,
    pub is_day: bool,
    /// Hora local del sitio ("2026-11-16T19:30")
    pub time: String,
    /// Viento en km/h y de dónde sopla (grados, 0 = del norte)
    pub wind: f32,
    pub wind_dir: f32,
    pub gusts: f32,
    /// Índice UV ahora
    pub uv: f32,
    /// Calidad del aire y polen (otra consulta; None si no hay datos)
    pub air: Option<Air>,
    pub days: Vec<Day>,
    /// Desde la hora actual en adelante
    pub hours: Vec<Hour>,
}

/// Calidad del aire y polen (Open-Meteo, servicio de calidad del aire)
#[derive(Debug, Clone, Default)]
pub struct Air {
    /// Índice europeo de calidad del aire (0-100+)
    pub aqi: Option<f32>,
    pub pm25: Option<f32>,
    /// Granos/m³ por tipo (clave del nombre), de mayor a menor: ("pollen.grass", 34.0)
    pub pollen: Vec<(&'static str, f32)>,
    /// Si el servicio da polen en este sitio (solo Europa)
    pub has_pollen: bool,
}

