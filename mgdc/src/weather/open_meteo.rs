//! Open-Meteo (gratis, sin cuenta ni clave): las consultas y cómo se leen

use std::{collections::HashMap, time::Duration};

use serde::Deserialize;

use super::{Air, Day, Hour, Weather};

#[derive(Deserialize)]
struct Raw {
    current: RawCurrent,
    hourly: RawHourly,
    daily: RawDaily,
}

#[derive(Deserialize)]
struct RawCurrent {
    time: String,
    temperature_2m: f32,
    weather_code: u8,
    is_day: u8,
    #[serde(default)]
    wind_speed_10m: Option<f32>,
    #[serde(default)]
    wind_direction_10m: Option<f32>,
    #[serde(default)]
    wind_gusts_10m: Option<f32>,
    #[serde(default)]
    uv_index: Option<f32>,
}

#[derive(Deserialize)]
struct RawHourly {
    time: Vec<String>,
    temperature_2m: Vec<Option<f32>>,
    weather_code: Vec<Option<u8>>,
    precipitation_probability: Vec<Option<u8>>,
}

#[derive(Deserialize)]
struct RawDaily {
    time: Vec<String>,
    weather_code: Vec<Option<u8>>,
    temperature_2m_max: Vec<Option<f32>>,
    temperature_2m_min: Vec<Option<f32>>,
    precipitation_probability_max: Vec<Option<u8>>,
    #[serde(default)]
    uv_index_max: Vec<Option<f32>>,
    #[serde(default)]
    sunrise: Vec<Option<String>>,
    #[serde(default)]
    sunset: Vec<Option<String>>,
}

/// Convierte la respuesta de Open-Meteo
pub fn parse(json: &str) -> Result<Weather, String> {
    let r: Raw = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let days = (0..r.daily.time.len())
        .map(|i| Day {
            date: r.daily.time[i].clone(),
            code: r.daily.weather_code.get(i).copied().flatten().unwrap_or(0),
            max: r.daily.temperature_2m_max.get(i).copied().flatten().unwrap_or(0.0),
            min: r.daily.temperature_2m_min.get(i).copied().flatten().unwrap_or(0.0),
            rain: r.daily.precipitation_probability_max.get(i).copied().flatten().unwrap_or(0),
            uv_max: r.daily.uv_index_max.get(i).copied().flatten().unwrap_or(0.0),
            sunrise: r.daily.sunrise.get(i).cloned().flatten().and_then(|t| t.get(11..16).map(String::from)).unwrap_or_default(),
            sunset: r.daily.sunset.get(i).cloned().flatten().and_then(|t| t.get(11..16).map(String::from)).unwrap_or_default(),
        })
        .collect();
    // Las horas desde la actual ("2026-11-16T19:30" → desde "2026-11-16T19:00")
    let now_hour = format!("{}:00", r.current.time.get(..13).unwrap_or(""));
    let hours = (0..r.hourly.time.len())
        .filter(|&i| r.hourly.time[i] >= now_hour)
        .map(|i| Hour {
            time: r.hourly.time[i].clone(),
            temp: r.hourly.temperature_2m.get(i).copied().flatten().unwrap_or(0.0),
            code: r.hourly.weather_code.get(i).copied().flatten().unwrap_or(0),
            rain: r.hourly.precipitation_probability.get(i).copied().flatten().unwrap_or(0),
        })
        .take(24)
        .collect();
    Ok(Weather {
        temp: r.current.temperature_2m,
        code: r.current.weather_code,
        is_day: r.current.is_day == 1,
        time: r.current.time.clone(),
        wind: r.current.wind_speed_10m.unwrap_or(0.0),
        wind_dir: r.current.wind_direction_10m.unwrap_or(0.0),
        gusts: r.current.wind_gusts_10m.unwrap_or(0.0),
        uv: r.current.uv_index.unwrap_or(0.0),
        air: None,
        days,
        hours,
    })
}

#[derive(Deserialize)]
struct RawAir {
    current: HashMap<String, serde_json::Value>,
}

/// Convierte la respuesta del servicio de calidad del aire
pub fn parse_air(json: &str) -> Result<Air, String> {
    let r: RawAir = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let num = |k: &str| r.current.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
    const KINDS: [(&str, &str); 6] = [
        ("grass_pollen", "pollen.grass"),
        ("olive_pollen", "pollen.olive"),
        ("birch_pollen", "pollen.birch"),
        ("alder_pollen", "pollen.alder"),
        ("mugwort_pollen", "pollen.mugwort"),
        ("ragweed_pollen", "pollen.ragweed"),
    ];
    let has_pollen = KINDS.iter().any(|(k, _)| num(k).is_some());
    let mut pollen: Vec<(&'static str, f32)> = KINDS.iter().map(|(k, n)| (*n, num(k).unwrap_or(0.0))).collect();
    pollen.sort_by(|a, b| b.1.total_cmp(&a.1));
    Ok(Air { aqi: num("european_aqi"), pm25: num("pm2_5"), pollen, has_pollen })
}

fn url(lat: f64, lon: f64) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}\
         &current=temperature_2m,weather_code,is_day,wind_speed_10m,wind_direction_10m,wind_gusts_10m,uv_index\
         &hourly=temperature_2m,weather_code,precipitation_probability\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,uv_index_max,sunrise,sunset\
         &timezone=auto&forecast_days=5"
    )
}

/// Página para ver la previsión de un sitio en el navegador: Windy, con el
/// mapa centrado en el sitio y su previsión detallada abierta al lado (la web
/// sale en el idioma del navegador). Las coordenadas llevan siempre decimales,
/// como pide Windy.
pub fn web_url(lat: f64, lon: f64) -> String {
    format!("https://www.windy.com/{lat:.4}/{lon:.4}?{lat:.4},{lon:.4},9")
}

fn air_url(lat: f64, lon: f64) -> String {
    format!(
        "https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat:.4}&longitude={lon:.4}\
         &current=european_aqi,pm2_5,alder_pollen,birch_pollen,grass_pollen,mugwort_pollen,olive_pollen,ragweed_pollen\
         &timezone=auto"
    )
}

pub(super) fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub(super) fn get_text(url: &str) -> Result<String, String> {
    ureq::get(url).timeout(Duration::from_secs(10)).call().map_err(|e| e.to_string())?.into_string().map_err(|e| e.to_string())
}

pub(super) fn fetch(lat: f64, lon: f64) -> Result<Weather, String> {
    let mut w = parse(&get_text(&url(lat, lon))?)?;
    // Si falla la calidad del aire, el resto se enseña igual
    match get_text(&air_url(lat, lon)).and_then(|t| parse_air(&t)) {
        Ok(a) => w.air = Some(a),
        Err(e) => log::warn!("Tiempo: calidad del aire: {e}"),
    }
    Ok(w)
}
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"latitude":40.4,"longitude":-3.7,"timezone":"Europe/Madrid",
      "current":{"time":"2026-11-16T19:30","interval":900,"temperature_2m":12.3,"weather_code":2,"is_day":0},
      "hourly":{"time":["2026-11-16T18:00","2026-11-16T19:00","2026-11-16T20:00"],"temperature_2m":[13.0,12.5,11.8],"weather_code":[1,2,61],"precipitation_probability":[0,5,40]},
      "daily":{"time":["2026-11-16","2026-11-17"],"weather_code":[2,61],"temperature_2m_max":[15.2,11.0],"temperature_2m_min":[4.1,6.0],"precipitation_probability_max":[10,80]}}"#;

    #[test]
    fn lee_open_meteo() {
        let w = parse(SAMPLE).unwrap();
        assert_eq!(w.code, 2);
        assert!(!w.is_day);
        assert_eq!(w.days.len(), 2);
        assert_eq!(w.days[1].rain, 80);
        // Las horas empiezan en la actual
        assert_eq!(w.hours.len(), 2);
        assert_eq!(w.hours[0].time, "2026-11-16T19:00");
        let a = parse_air(r#"{"current":{"time":"2026-11-16T19:00","european_aqi":27,"pm10":12.5,"pm2_5":7.1,"grass_pollen":34.0,"olive_pollen":2.0,"birch_pollen":null,"alder_pollen":0.0,"mugwort_pollen":0.0,"ragweed_pollen":6.0}}"#).unwrap();
        assert_eq!(a.aqi, Some(27.0));
        assert!(a.has_pollen);
        assert_eq!(a.pollen[0], ("pollen.grass", 34.0));
        assert_eq!(web_url(40.0, -3.7038), "https://www.windy.com/40.0000/-3.7038?40.0000,-3.7038,9");
        assert_eq!(url_encode("Alcalá de"), "Alcal%C3%A1%20de");
    }
}
