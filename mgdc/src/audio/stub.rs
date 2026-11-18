//! Fuera de Windows no hay audio que controlar: todo devuelve "nada".

use super::{Level, NowPlaying};

pub fn apps() -> Vec<String> {
    vec![]
}
pub fn level(_target: &str) -> Option<Level> {
    None
}
pub fn set_volume(_target: &str, _volume: u8) -> Option<Level> {
    None
}
pub fn set_mute(_target: &str, _muted: bool) -> Option<Level> {
    None
}
pub fn now_playing() -> Option<NowPlaying> {
    None
}
pub fn peak(_process: &str) -> Option<f32> {
    None
}
