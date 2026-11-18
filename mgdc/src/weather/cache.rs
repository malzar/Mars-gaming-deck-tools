//! Lo que ya se sabe del tiempo de cada sitio. Se pide una vez cada 15 minutos
//! por sitio y se guarda en memoria; todas las teclas y ruedas del mismo sitio
//! usan la misma consulta.

use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

use super::{Weather, open_meteo::fetch};


/// Cada cuánto se vuelve a preguntar
const EVERY: Duration = Duration::from_secs(15 * 60);
/// Si falla, se reintenta antes
const RETRY: Duration = Duration::from_secs(60);

struct Entry {
    data: Option<Weather>,
    at: Instant,
    ok: bool,
}

static CACHE: LazyLock<Mutex<HashMap<String, Entry>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static FETCHING: LazyLock<Mutex<std::collections::HashSet<String>>> = LazyLock::new(Default::default);

fn key(lat: f64, lon: f64) -> String {
    format!("{lat:.2},{lon:.2}")
}

/// Lo último que se sabe de un sitio (sin esperar). Si toca, pide datos
/// nuevos en segundo plano.
pub async fn get(lat: f64, lon: f64) -> Option<Weather> {
    let k = key(lat, lon);
    let (data, stale) = match CACHE.lock().await.get(&k) {
        Some(e) => (e.data.clone(), e.at.elapsed() > if e.ok { EVERY } else { RETRY }),
        None => (None, true),
    };
    if stale && FETCHING.lock().await.insert(k.clone()) {
        tokio::spawn(async move {
            let r = tokio::task::spawn_blocking(move || fetch(lat, lon)).await.unwrap_or_else(|e| Err(e.to_string()));
            let mut c = CACHE.lock().await;
            let old = c.get(&k).and_then(|e| e.data.clone());
            match r {
                Ok(w) => {
                    c.insert(k.clone(), Entry { data: Some(w), at: Instant::now(), ok: true });
                }
                Err(e) => {
                    log::warn!("Tiempo: no se pudo consultar {k}: {e}");
                    c.insert(k.clone(), Entry { data: old, at: Instant::now(), ok: false });
                }
            }
            FETCHING.lock().await.remove(&k);
        });
    }
    data
}
