//! Fuera de Windows no hay audio que controlar: todo devuelve "nada".

use super::{Level, NowPlaying, Output, OutputKind};

pub fn outputs() -> Vec<Output> {
    vec![]
}
pub fn set_default_output(_id: &str) -> Result<(), String> {
    Err("solo en Windows".into())
}
pub fn default_output_kind() -> Option<OutputKind> {
    None
}
pub fn forget_devices() {}
pub fn app_icon(_process: &str) -> Option<(u32, Vec<u8>)> {
    None
}
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
