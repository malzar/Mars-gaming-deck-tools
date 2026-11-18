//! Animaciones de arranque y de cierre.
//!
//! Se pintan en las teclas (no en el fondo: el fondo se graba en la memoria del
//! deck, tarda segundos y desgastaría la memoria). Cada fotograma calcula un
//! color por tecla y solo reenvía las teclas que cambian.
//!
//! - `patterns`: qué teclas se encienden en cada paso (lineal, flecha…)
//! - `trail`: la estela de los pasos (o el modo fijo)
//! - `steps`, `sweep`, `fade`, `gif`: cómo se reproduce cada tipo
//! - `screen`: la rejilla del deck (teclas y pantallas de las ruedas)
//! - `color`: paleta y colores al azar
//! - `dark`: el deck en negro al arrancar
//! - `ending`: cómo se queda el deck al cerrar (fondo, logo, foto o apagado)

mod color;
mod dark;
mod ending;
mod fade;
mod gif;
mod patterns;
mod screen;
mod steps;
mod sweep;
mod trail;

use std::{
    collections::HashSet,
    sync::LazyLock,
    time::{Duration, Instant},
};

use mirajazz::{device::Device, error::MirajazzError};
use tokio::sync::Mutex;

use crate::{
    family::Model,
    settings::{AnimKind, Animation},
};

pub use dark::{blackout, end_dark, painted, start_dark};
pub use ending::{Ending, clear, finish_closing, photo_path, preview_ending};
pub use gif::{gif_path, validate_gif};

/// Duración de las animaciones: de 0,2 a 2 s
pub const MIN_MS: u32 = 200;
pub const MAX_MS: u32 = 2000;
/// Tiempo mínimo entre fotogramas (unos 16 por segundo como mucho)
const FRAME: Duration = Duration::from_millis(60);

/// Decks con una animación en marcha: mientras tanto se ignoran las imágenes
/// que manda OpenDeck (se repintan todas al terminar)
static BUSY: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub async fn busy(id: &str) -> bool {
    BUSY.lock().await.contains(id)
}

pub async fn set_busy(id: &str, on: bool) {
    let mut b = BUSY.lock().await;
    if on {
        b.insert(id.to_string());
    } else {
        b.remove(id);
    }
}

/// Reproduce una animación. `closing` = animación de cierre (va "al revés").
/// `brightness` es el brillo normal de la pantalla, que el fundido usa y restaura.
pub async fn play(device: &Device, model: &Model, id: &str, anim: &Animation, closing: bool, brightness: u8) -> Result<(), MirajazzError> {
    let start = Instant::now();
    // Las animaciones también respetan la calibración de iconos
    let cal = crate::settings::for_device(id).await.calibration;
    let frames = match &anim.kind {
        AnimKind::None => return Ok(()),
        AnimKind::Gif => gif::play_gif(device, model, &cal, id, closing).await?,
        // Retiradas: no hacen nada
        AnimKind::Rows | AnimKind::Spiral => 0,
        AnimKind::Fade => fade::play_fade(device, model, &cal, anim, closing, brightness).await?,
        kind => match patterns::of(kind) {
            Some(pattern) => steps::play_steps(device, model, &cal, anim, pattern(model), !closing).await?,
            None => sweep::play_effect(device, model, &cal, anim, closing).await?,
        },
    };
    log::info!(
        "Animación {:?} ({}) en {id}: {frames} fotogramas en {} ms",
        anim.kind,
        if closing { "cierre" } else { "arranque" },
        start.elapsed().as_millis()
    );
    Ok(())
}
