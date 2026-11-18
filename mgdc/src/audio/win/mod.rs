//! Audio en Windows (COM): volumen general y por programa, y lo que suena
//!
//! - `volume`: altavoces, sesiones de cada programa y sus nombres
//! - `playing`: qué programas suenan, sus ventanas y el volumen real (pico)

mod playing;
mod volume;

pub use playing::{now_playing, peak, playing_processes};
pub use volume::{apps, level, process_name, set_mute, set_volume};

use std::{cell::RefCell, collections::HashMap, time::{Duration, Instant}};

use windows::Win32::{
    Media::Audio::{Endpoints::IAudioEndpointVolume, IAudioSessionManager2},
    System::Com::{COINIT_MULTITHREADED, CoInitializeEx},
};

/// COM tiene que estar iniciado en cada hilo que lo use (repetirlo no pasa nada)
pub(super) fn com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

// Abrir los objetos de audio de Windows cuesta; se guardan un rato. Todo el
// audio va por un único hilo (ver `audio_actions::worker`), así que basta
// con guardarlos por hilo. Se renuevan cada pocos segundos por si cambias
// de altavoces o auriculares.
pub(super) const KEEP: Duration = Duration::from_secs(5);

thread_local! {
    pub(super) static ENDPOINT: RefCell<Option<(IAudioEndpointVolume, Instant)>> = const { RefCell::new(None) };
    pub(super) static SESSIONS: RefCell<Option<(IAudioSessionManager2, Instant)>> = const { RefCell::new(None) };
    pub(super) static NAMES: RefCell<(HashMap<u32, String>, Option<Instant>)> = RefCell::new((HashMap::new(), None));
}
