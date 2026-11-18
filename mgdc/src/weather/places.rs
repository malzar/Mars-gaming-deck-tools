//! Buscar localidades por nombre (cualquier pueblo o ciudad del mundo)

use serde::Deserialize;

use super::open_meteo::{get_text, url_encode};

/// Un sitio encontrado al buscar una localidad
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Place {
    pub name: String,
    /// Provincia o región y país, para distinguir sitios con el mismo nombre
    pub region: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Deserialize)]
struct RawPlaces {
    #[serde(default)]
    results: Vec<RawPlace>,
}

#[derive(Deserialize)]
struct RawPlace {
    name: String,
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    admin1: Option<String>,
    #[serde(default)]
    admin2: Option<String>,
    #[serde(default)]
    country: Option<String>,
}

/// Convierte la respuesta del buscador de localidades de Open-Meteo
pub fn parse_places(json: &str) -> Result<Vec<Place>, String> {
    let r: RawPlaces = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(r.results
        .into_iter()
        .map(|p| {
            // Provincia (o región) y país, sin repetir el propio nombre
            let mut parts: Vec<String> = vec![];
            for x in [p.admin2, p.admin1, p.country].into_iter().flatten() {
                if x != p.name && !parts.contains(&x) {
                    parts.push(x);
                }
            }
            if parts.len() > 2 {
                parts.remove(1);
            }
            Place { name: p.name, region: parts.join(", "), lat: p.latitude, lon: p.longitude }
        })
        .collect())
}

/// Busca localidades por nombre (cualquier pueblo o ciudad del mundo), con
/// los nombres en el idioma pedido
pub fn search_places(query: &str, lang: &str) -> Result<Vec<Place>, String> {
    let q = query.trim();
    if q.chars().count() < 2 {
        return Ok(vec![]);
    }
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=10&format=json&language={}",
        url_encode(q),
        url_encode(lang.get(..2).unwrap_or("es"))
    );
    parse_places(&get_text(&url)?)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_localidades() {
        let p = parse_places(r#"{"results":[{"name":"Alcalá de Henares","latitude":40.48,"longitude":-3.36,"admin1":"Comunidad de Madrid","admin2":"Madrid","country":"España"}]}"#).unwrap();
        assert_eq!(p[0].region, "Madrid, España");
        assert_eq!(parse_places("{}").unwrap(), vec![]);
    }
}
