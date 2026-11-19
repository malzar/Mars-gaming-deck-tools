//! Iconos de los programas que suenan (para la rueda de música), guardados
//! para no sacarlos del .exe cada vez

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};

use super::worker::run_blocking;
use crate::player::Pixels;

/// Si no se pudo sacar, cuándo se vuelve a probar
const RETRY: Duration = Duration::from_secs(30);

/// Programa → sus píxeles (o `None` si no se pudo, y cuándo se probó)
type Icon = (Option<Arc<Pixels>>, Instant);
static ICONS: LazyLock<Mutex<HashMap<String, Icon>>> = LazyLock::new(Default::default);

/// Saca el icono del programa si aún no se tiene
pub(super) async fn ensure(process: &str) {
    let process = process.trim().to_lowercase();
    let known = ICONS.lock().unwrap().get(&process).is_some_and(|(px, at)| px.is_some() || at.elapsed() < RETRY);
    if process.is_empty() || known {
        return;
    }
    let p = process.clone();
    let px = run_blocking(move || crate::audio::app_icon(&p))
        .await
        .flatten()
        .filter(|(size, rgba)| rgba.len() == (size * size * 4) as usize)
        .map(|(size, rgba)| Arc::new(Pixels { size, rgba }));
    log::info!("Audio: icono de {process}: {}", if px.is_some() { "sí" } else { "no" });
    ICONS.lock().unwrap().insert(process, (px, Instant::now()));
}

/// El icono del programa, si ya se tiene
pub(super) fn get(process: &str) -> Option<Arc<Pixels>> {
    ICONS.lock().unwrap().get(&process.trim().to_lowercase()).and_then(|(px, _)| px.clone())
}
