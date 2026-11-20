//! Avisos oficiales de meteorología (amarillo, naranja, rojo) del organismo de
//! cada país.
//!
//! La lista de organismos es la de la Organización Meteorológica Mundial
//! (cada uno con su canal de avisos en formato CAP); se descarga como mucho
//! una vez al día y se guarda en disco. Cada canal se lee cada 15 minutos y
//! cada aviso (un fichero CAP que no cambia) se descarga una sola vez.
//!
//! Un aviso vale para el sitio si su zona lo contiene (polígono o círculo);
//! si el aviso no trae la zona dibujada, por el nombre de la zona.

use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
    time::{Duration, Instant, SystemTime},
};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::Mutex;

use super::open_meteo::get_text;

const SOURCES_URL: &str = "https://severeweather.wmo.int/json/sources.json";
const SOURCES_EVERY: Duration = Duration::from_secs(24 * 3600);
const FEED_EVERY: Duration = Duration::from_secs(15 * 60);
const RETRY: Duration = Duration::from_secs(5 * 60);
/// Avisos (ficheros CAP) que se descargan como mucho en cada lectura
const MAX_NEW_CAPS: usize = 80;
/// Avisos que empiezan dentro de este tiempo ya se muestran
const AHEAD: chrono::TimeDelta = chrono::TimeDelta::hours(12);

/// Un organismo de avisos (de la lista de la OMM)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Source {
    pub id: String,
    /// "Spain: Agencia Estatal de Meteorología"
    pub name: String,
    pub feed: String,
    /// Código del país ("es") y siglas ("AEMET"), para buscarlo
    pub country: String,
    pub abbrev: String,
    /// Si la OMM dice que su canal funciona
    pub ok: bool,
    #[serde(skip)]
    pub at: Option<(f64, f64)>,
}

/// Nivel de un aviso
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Yellow,
    Orange,
    Red,
}

/// De qué es un aviso
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hazard {
    Rain,
    Heat,
    Cold,
    Wind,
    Storm,
    Snow,
    Other,
}

/// Un aviso oficial que afecta al sitio
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    pub hazard: Hazard,
    pub level: Level,
    /// «Aviso amarillo por vientos»
    pub event: String,
    pub expires: Option<DateTime<Utc>>,
    /// Para no repetir sonido ni volver a enseñar uno quitado
    pub id: String,
}

// ---------------------------------------------------------------- lista --

fn parse_sources(json: &str) -> Vec<Source> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    let list = v.as_array().or_else(|| v["sources"].as_array()).cloned().unwrap_or_default();
    list.iter()
        // Cada elemento viene como {"source": {...}}
        .map(|e| if e["source"].is_object() { &e["source"] } else { e })
        .filter_map(|s| {
            let feed = s["capAlertFeed"].as_str()?.trim().to_string();
            if feed.is_empty() {
                return None;
            }
            let name = s["byLanguage"]
                .as_array()
                .and_then(|l| l.iter().find_map(|x| x["name"].as_str()))
                .map(String::from)
                .unwrap_or_else(|| s["countryName"].as_str().unwrap_or("?").to_string());
            let at = match &s["latlng"] {
                serde_json::Value::String(t) => t.split_once(',').and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?))),
                serde_json::Value::Array(a) => Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)),
                _ => None,
            };
            let id = s["sourceId"].as_str().unwrap_or(&feed).to_string();
            Some(Source {
                country: id.split('-').next().unwrap_or("").to_lowercase(),
                abbrev: s["authorityAbbrev"].as_str().unwrap_or("").trim().to_string(),
                id,
                name,
                ok: s["capAlertFeedStatus"].as_str().is_none_or(|x| x == "operating"),
                feed,
                at,
            })
        })
        .collect()
}

static SOURCES: LazyLock<Mutex<(Vec<Source>, Option<Instant>)>> = LazyLock::new(Default::default);

fn sources_file() -> Option<std::path::PathBuf> {
    Some(crate::app_dir()?.join("cache").join("alert-sources.json"))
}

/// La lista guardada en disco (y si es de hoy)
fn saved_sources() -> (Vec<Source>, bool) {
    let Some(f) = sources_file() else { return (vec![], false) };
    let fresh = f.metadata().ok().and_then(|m| m.modified().ok()).is_some_and(|t| SystemTime::now().duration_since(t).unwrap_or_default() < SOURCES_EVERY);
    (std::fs::read_to_string(&f).map(|t| parse_sources(&t)).unwrap_or_default(), fresh)
}

/// Baja la lista de la OMM y la guarda en disco
fn download_sources() -> Result<Vec<Source>, String> {
    let text = get_text(SOURCES_URL)?;
    let l = parse_sources(&text);
    log::info!("Avisos: lista de la OMM bajada, {} centros ({} bytes)", l.len(), text.len());
    if l.is_empty() {
        return Err(crate::i18n::t("weather.sources_empty").into());
    }
    if let Some(f) = sources_file() {
        let _ = std::fs::create_dir_all(f.parent().unwrap());
        if let Err(e) = std::fs::write(&f, &text) {
            log::warn!("Avisos: no se pudo guardar {}: {e}", f.display());
        }
    }
    Ok(l)
}

/// La lista de disco si es de hoy; si no (o `force`), la de la OMM. Si no se
/// puede bajar, la vieja de disco, si la hay.
fn load_sources(force: bool) -> Result<Vec<Source>, String> {
    let (saved, fresh) = saved_sources();
    if fresh && !force && !saved.is_empty() {
        return Ok(saved);
    }
    match download_sources() {
        Ok(l) => Ok(l),
        Err(e) => {
            log::warn!("Avisos: no se pudo bajar la lista de la OMM: {e}");
            if saved.is_empty() { Err(e) } else { Ok(saved) }
        }
    }
}

/// Todos los organismos de avisos (de memoria; la primera vez, de disco o
/// de la OMM)
pub async fn sources() -> Vec<Source> {
    {
        let s = SOURCES.lock().await;
        if s.1.is_some_and(|t| t.elapsed() < if s.0.is_empty() { RETRY } else { SOURCES_EVERY }) {
            return s.0.clone();
        }
    }
    reload_sources(false).await.unwrap_or_default()
}

/// Vuelve a cargar la lista (`force`: bajándola de la OMM aunque haya una de
/// hoy); el error se puede enseñar
pub async fn reload_sources(force: bool) -> Result<Vec<Source>, String> {
    let r = tokio::task::spawn_blocking(move || load_sources(force)).await.unwrap_or_else(|e| Err(e.to_string()));
    *SOURCES.lock().await = (r.clone().unwrap_or_default(), Some(Instant::now()));
    r
}

/// Al arrancar el plugin: deja la lista lista (de disco o de la OMM, como
/// mucho una vez al día)
pub async fn preload_sources() {
    match reload_sources(false).await {
        Ok(l) => log::info!("Avisos: {} centros de avisos", l.len()),
        Err(e) => log::warn!("Avisos: sin lista de centros: {e}"),
    }
}

fn km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) = (a.0.to_radians(), a.1.to_radians(), b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    12_742.0 * h.sqrt().asin()
}

/// El id del organismo más cercano a un sitio
pub fn nearest_source(list: &[Source], lat: f64, lon: f64) -> Option<String> {
    pick(list, "", (lat, lon)).map(|s| s.id.clone())
}

/// El organismo elegido (`id`), o si está vacío el más cercano al sitio
pub fn pick<'a>(list: &'a [Source], id: &str, at: (f64, f64)) -> Option<&'a Source> {
    if !id.is_empty() {
        return list.iter().find(|s| s.id == id);
    }
    list.iter().filter(|s| s.ok && s.at.is_some()).min_by(|a, b| km(a.at.unwrap(), at).total_cmp(&km(b.at.unwrap(), at)))
}

// ------------------------------------------------------------- canales --

/// Un aviso leído de un CAP (aún sin mirar si es del sitio)
#[derive(Debug, Clone, PartialEq)]
struct Cap {
    id: String,
    event: String,
    level: Option<Level>,
    onset: Option<DateTime<Utc>>,
    expires: Option<DateTime<Utc>>,
    /// Zonas: polígonos (lat, lon) y círculos (centro, km), y sus nombres
    polygons: Vec<Vec<(f64, f64)>>,
    circles: Vec<((f64, f64), f64)>,
    names: Vec<String>,
}

fn time(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s.trim()).ok().map(|t| t.with_timezone(&Utc))
}

fn level_of(severity: &str, awareness: &str) -> Option<Level> {
    let a = awareness.to_lowercase();
    if a.contains("red") || a.contains("rojo") {
        return Some(Level::Red);
    }
    if a.contains("orange") || a.contains("naranja") {
        return Some(Level::Orange);
    }
    if a.contains("yellow") || a.contains("amarillo") {
        return Some(Level::Yellow);
    }
    match severity.trim() {
        "Extreme" => Some(Level::Red),
        "Severe" => Some(Level::Orange),
        "Moderate" => Some(Level::Yellow),
        _ => None,
    }
}

/// De qué va un aviso, por las palabras de su nombre (español, inglés y
/// algunas más)
pub fn hazard_of(event: &str) -> Hazard {
    let e = event.to_lowercase();
    let any = |w: &[&str]| w.iter().any(|x| e.contains(x));
    if any(&["tormenta", "thunder", "orage", "gewitter", "temporale"]) {
        Hazard::Storm
    } else if any(&["nieve", "nevada", "snow", "neige", "schnee", "hielo", "ice", "helada", "frost", "glace"]) {
        Hazard::Snow
    } else if any(&["máxima", "maxima", "calor", "heat", "canicule", "hitze", "high temp"]) {
        Hazard::Heat
    } else if any(&["mínima", "minima", "frío", "frio", "cold", "froid", "kälte", "low temp"]) {
        Hazard::Cold
    } else if any(&["viento", "wind", "vent", "rachas", "gust"]) {
        Hazard::Wind
    } else if any(&["lluvia", "precipita", "rain", "pluie", "regen", "inunda", "flood"]) {
        Hazard::Rain
    } else {
        Hazard::Other
    }
}

fn child_text<'a>(n: roxmltree::Node<'a, 'a>, name: &str) -> Option<&'a str> {
    n.children().find(|c| c.tag_name().name() == name).and_then(|c| c.text())
}

fn points(s: &str) -> Vec<(f64, f64)> {
    s.split_whitespace()
        .filter_map(|p| {
            let (a, b) = p.split_once(',')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect()
}

/// Lee un fichero CAP (la parte en el idioma `lang` si la hay)
fn parse_cap(xml: &str, id: &str, lang: &str) -> Option<Cap> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let infos: Vec<_> = doc.descendants().filter(|n| n.tag_name().name() == "info").collect();
    let info = infos.iter().find(|i| child_text(**i, "language").is_some_and(|l| l.to_lowercase().starts_with(lang))).or(infos.first())?;
    let awareness: String = info
        .children()
        .filter(|c| c.tag_name().name() == "parameter")
        .filter(|p| child_text(*p, "valueName").is_some_and(|n| { let n = n.to_lowercase(); n.contains("awareness_level") || n.contains("nivel") }))
        .filter_map(|p| child_text(p, "value"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut cap = Cap {
        id: id.to_string(),
        event: child_text(*info, "headline").or(child_text(*info, "event")).unwrap_or("").trim().to_string(),
        level: level_of(child_text(*info, "severity").unwrap_or(""), &awareness),
        onset: child_text(*info, "onset").or(child_text(*info, "effective")).and_then(time),
        expires: child_text(*info, "expires").and_then(time),
        polygons: vec![],
        circles: vec![],
        names: vec![],
    };
    if let Some(ev) = child_text(*info, "event")
        && hazard_of(&cap.event) == Hazard::Other
    {
        cap.event = ev.trim().to_string();
    }
    for area in info.children().filter(|c| c.tag_name().name() == "area") {
        if let Some(d) = child_text(area, "areaDesc") {
            cap.names.push(d.trim().to_string());
        }
        for c in area.children() {
            match c.tag_name().name() {
                "polygon" => cap.polygons.push(points(c.text().unwrap_or(""))),
                "circle" => {
                    let t = c.text().unwrap_or("");
                    if let Some((p, r)) = t.trim().split_once(' ')
                        && let (Some(&c), Ok(r)) = (points(p).first(), r.parse::<f64>())
                    {
                        cap.circles.push((c, r));
                    }
                }
                _ => {}
            }
        }
    }
    Some(cap)
}

/// Enlaces de un canal RSS o Atom, y los avisos que ya trae dentro (algunos
/// canales, como los de MeteoAlarm, ponen el CAP resumido en cada entrada)
fn parse_feed(xml: &str) -> (Vec<String>, Vec<Cap>) {
    let Ok(doc) = roxmltree::Document::parse(xml) else { return (vec![], vec![]) };
    let mut links = vec![];
    let mut inline = vec![];
    for e in doc.descendants().filter(|n| matches!(n.tag_name().name(), "item" | "entry")) {
        let link = e
            .children()
            .filter(|c| c.tag_name().name() == "link")
            .find_map(|l| l.attribute("href").map(String::from).or_else(|| l.text().map(|t| t.trim().to_string())))
            .filter(|l| !l.is_empty());
        // ¿Trae el aviso dentro?
        let cap_child = |name: &str| e.children().find(|c| c.tag_name().name() == name && c.tag_name().namespace().is_some_and(|ns| ns.contains("cap"))).and_then(|c| c.text());
        if let Some(event) = cap_child("event") {
            inline.push(Cap {
                id: link.clone().unwrap_or_else(|| event.to_string()),
                event: event.trim().to_string(),
                level: level_of(cap_child("severity").unwrap_or(""), cap_child("awareness_level").unwrap_or("")),
                onset: cap_child("onset").or(cap_child("effective")).and_then(time),
                expires: cap_child("expires").and_then(time),
                polygons: vec![],
                circles: vec![],
                names: cap_child("areaDesc").map(|d| vec![d.trim().to_string()]).unwrap_or_default(),
            });
        } else if let Some(l) = link {
            links.push(l);
        }
    }
    (links, inline)
}

fn inside(poly: &[(f64, f64)], p: (f64, f64)) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
    }
    c
}

fn plain(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            c => c,
        })
        .collect()
}

/// Si el aviso es para el sitio
fn applies(c: &Cap, at: (f64, f64), city: &str) -> bool {
    if !c.polygons.is_empty() || !c.circles.is_empty() {
        return c.polygons.iter().any(|p| p.len() > 2 && inside(p, at)) || c.circles.iter().any(|(m, r)| km(*m, at) <= *r);
    }
    let city = plain(city.split(',').next().unwrap_or(city).trim());
    !city.is_empty() && c.names.iter().any(|n| plain(n).contains(&city))
}

fn to_warning(c: &Cap, now: DateTime<Utc>) -> Option<Warning> {
    let level = c.level?;
    if c.expires.is_some_and(|e| e < now) || c.onset.is_some_and(|o| o > now + AHEAD) {
        return None;
    }
    Some(Warning { hazard: hazard_of(&c.event), level, event: c.event.clone(), expires: c.expires, id: c.id.clone() })
}

// ------------------------------------------------------------ memoria --

#[derive(Default)]
struct FeedState {
    caps: Vec<Cap>,
    at: Option<Instant>,
    ok: bool,
    /// El último error al leerlo
    error: String,
}

static FEEDS: LazyLock<Mutex<HashMap<String, FeedState>>> = LazyLock::new(Default::default);
static FETCHING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);
/// Ficheros CAP ya leídos (no cambian)
static CAPS: LazyLock<std::sync::Mutex<HashMap<String, Option<Cap>>>> = LazyLock::new(Default::default);

fn read_feed(feed: &str, lang: &str) -> Result<Vec<Cap>, String> {
    let (links, mut caps) = parse_feed(&get_text(feed)?);
    let mut new = 0;
    for l in links {
        if let Some(c) = CAPS.lock().unwrap().get(&l).cloned() {
            caps.extend(c);
            continue;
        }
        if new >= MAX_NEW_CAPS {
            continue;
        }
        new += 1;
        let c = get_text(&l).ok().and_then(|x| parse_cap(&x, &l, lang));
        CAPS.lock().unwrap().insert(l, c.clone());
        caps.extend(c);
    }
    // Que no crezca sin fin
    let mut all = CAPS.lock().unwrap();
    if all.len() > 2000 {
        all.clear();
    }
    Ok(caps)
}

/// Cómo va la lectura del centro de avisos (para la ventana de ajustes)
#[derive(Debug, Clone, Serialize)]
pub struct FeedStatus {
    /// "ok", "error" o "loading" (aún leyendo)
    pub state: &'static str,
    /// Nombre del centro
    pub name: String,
    /// Avisos leídos en su canal y los que tocan al sitio
    pub total: usize,
    pub here: usize,
    /// Hace cuántos minutos se leyó
    pub minutes: u64,
    pub error: String,
}

/// Cómo va el centro de avisos elegido (y si aún no se ha leído, empieza)
pub async fn status(source: &str, lat: f64, lon: f64, city: &str) -> Option<FeedStatus> {
    if source == "none" {
        return None;
    }
    let here = official(source, lat, lon, city).await.len();
    let list = sources().await;
    let src = pick(&list, source, (lat, lon))?;
    let feeds = FEEDS.lock().await;
    let base = FeedStatus { state: "loading", name: src.name.clone(), total: 0, here, minutes: 0, error: String::new() };
    Some(match feeds.get(&src.feed) {
        None => base,
        Some(f) => FeedStatus {
            state: if f.ok { "ok" } else { "error" },
            total: f.caps.len(),
            minutes: f.at.map_or(0, |t| t.elapsed().as_secs() / 60),
            error: f.error.clone(),
            ..base
        },
    })
}

/// Avisos oficiales para el sitio (sin esperar: lo último leído; si toca,
/// vuelve a leer el canal en segundo plano). `source`: id del organismo,
/// vacío = el más cercano, "none" = ninguno.
pub async fn official(source: &str, lat: f64, lon: f64, city: &str) -> Vec<Warning> {
    if source == "none" {
        return vec![];
    }
    let list = sources().await;
    let Some(src) = pick(&list, source, (lat, lon)) else { return vec![] };
    let feed = src.feed.clone();
    let (caps, stale) = match FEEDS.lock().await.get(&feed) {
        Some(f) => (f.caps.clone(), f.at.is_none_or(|t| t.elapsed() > if f.ok { FEED_EVERY } else { RETRY })),
        None => (vec![], true),
    };
    if stale && FETCHING.lock().await.insert(feed.clone()) {
        let lang = crate::i18n::code();
        tokio::spawn(async move {
            let f2 = feed.clone();
            let r = tokio::task::spawn_blocking(move || read_feed(&f2, &lang)).await.unwrap_or_else(|e| Err(e.to_string()));
            let mut feeds = FEEDS.lock().await;
            let old = feeds.remove(&feed).unwrap_or_default();
            let state = match r {
                Ok(caps) => {
                    log::info!("Avisos: {} avisos en {feed}", caps.len());
                    FeedState { caps, at: Some(Instant::now()), ok: true, error: String::new() }
                }
                Err(e) => {
                    log::warn!("Avisos: no se pudo leer {feed}: {e}");
                    FeedState { caps: old.caps, at: Some(Instant::now()), ok: false, error: e }
                }
            };
            feeds.insert(feed.clone(), state);
            FETCHING.lock().await.remove(&feed);
        });
    }
    let now = Utc::now();
    let mut out: Vec<Warning> = caps.iter().filter(|c| applies(c, (lat, lon), city)).filter_map(|c| to_warning(c, now)).collect();
    out.sort_by_key(|w| std::cmp::Reverse(w.level));
    out.dedup_by(|a, b| a.hazard == b.hazard && a.level == b.level);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<alert xmlns="urn:oasis:names:tc:emergency:cap:1.2"><identifier>x</identifier>
<info><language>en-GB</language><event>Moderate wind warning</event><severity>Moderate</severity>
<onset>2026-11-19T10:00:00+01:00</onset><expires>2026-11-19T22:00:00+01:00</expires><headline>Yellow warning. Wind</headline>
<area><areaDesc>Sierra de Madrid</areaDesc><polygon>40.0,-4.5 41.5,-4.5 41.5,-3.0 40.0,-3.0 40.0,-4.5</polygon></area></info>
<info><language>es-ES</language><event>Aviso amarillo de vientos</event><severity>Moderate</severity>
<onset>2026-11-19T10:00:00+01:00</onset><expires>2026-11-19T22:00:00+01:00</expires><headline>Aviso amarillo por rachas de viento</headline>
<parameter><valueName>AEMET-Meteoalerta nivel</valueName><value>amarillo</value></parameter>
<area><areaDesc>Sierra de Madrid</areaDesc><polygon>40.0,-4.5 41.5,-4.5 41.5,-3.0 40.0,-3.0 40.0,-4.5</polygon></area></info></alert>"#;

    #[test]
    fn lee_un_cap() {
        let c = parse_cap(CAP, "u", "es").unwrap();
        assert_eq!(c.event, "Aviso amarillo por rachas de viento");
        assert_eq!(c.level, Some(Level::Yellow));
        assert_eq!(hazard_of(&c.event), Hazard::Wind);
        assert!(applies(&c, (40.42, -3.70), ""));
        assert!(!applies(&c, (39.47, -0.38), ""));
        let now = time("2026-11-19T12:00:00Z").unwrap();
        assert!(to_warning(&c, now).is_some());
        assert!(to_warning(&c, time("2026-11-20T12:00:00Z").unwrap()).is_none());
        assert_eq!(parse_cap(CAP, "u", "en").unwrap().event, "Yellow warning. Wind");
    }

    #[test]
    fn lee_canales() {
        let rss = r#"<rss><channel><item><title>a</title><link>https://x/a.xml</link></item><item><link>https://x/b.xml</link></item></channel></rss>"#;
        assert_eq!(parse_feed(rss).0, vec!["https://x/a.xml", "https://x/b.xml"]);
        let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom" xmlns:cap="urn:oasis:names:tc:emergency:cap:1.2"><entry><link href="https://m/1"/>
            <cap:event>Thunderstorm</cap:event><cap:severity>Severe</cap:severity><cap:areaDesc>Madrid: Sierra</cap:areaDesc>
            <cap:expires>2099-01-01T00:00:00+00:00</cap:expires></entry></feed>"#;
        let (links, inline) = parse_feed(atom);
        assert!(links.is_empty());
        assert_eq!(inline[0].level, Some(Level::Orange));
        assert_eq!(hazard_of(&inline[0].event), Hazard::Storm);
        assert!(applies(&inline[0], (0.0, 0.0), "Madrid, España"));
        assert!(!applies(&inline[0], (0.0, 0.0), "Valencia"));
    }

    #[test]
    fn lista_de_la_omm() {
        let json = r#"{"sources":[{"source":{"sourceId":"es-aemet-es","byLanguage":[{"code":"en","name":"Spain: Agencia Estatal de Meteorología"}],"capAlertFeed":"https://aemet/rss.xml","capAlertFeedStatus":"operating","latlng":"40.4,-3.7"}},
            {"sourceId":"pt-ipma-pt","byLanguage":[{"name":"Portugal: IPMA"}],"capAlertFeed":"https://ipma/rss.xml","capAlertFeedStatus":"operating","latlng":[38.7,-9.1]},
            {"sourceId":"x","capAlertFeed":""}]}"#;
        let l = parse_sources(json);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].country.as_str(), l[1].country.as_str()), ("es", "pt"));
        assert_eq!(pick(&l, "", (41.6, -0.9)).unwrap().id, "es-aemet-es");
        assert_eq!(pick(&l, "", (39.0, -8.5)).unwrap().id, "pt-ipma-pt");
        assert_eq!(pick(&l, "pt-ipma-pt", (41.6, -0.9)).unwrap().id, "pt-ipma-pt");
        assert!(pick(&l, "nada", (0.0, 0.0)).is_none());
    }
}
