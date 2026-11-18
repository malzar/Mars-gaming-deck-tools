//! Gestos de la pantalla táctil del MSD-PRO.
//!
//! El deck manda los toques de la pantalla con el mismo evento que pulsar la
//! rueda de debajo, así que al leerlos se apunta aquí que ha sido un toque;
//! la rueda de perfiles lo mira para saber si fue pantalla o pulsador.
//!
//! Los deslizamientos a izquierda/derecha llegan como el giro de una rueda de
//! más (STRIP), sin decir dónde fue el dedo. Para saber a qué rueda van se
//! toca antes su zona, se suelta y se desliza (como un doble click): si la
//! rueda tiene el deslizamiento activado, el toque espera un momento por si
//! llega el deslizamiento. Probado en el aparato: a velocidad normal el
//! deslizamiento llega 0,4–0,9 s después del toque; muy rápido el deck pierde
//! el toque, y muy despacio a veces da la dirección al revés.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use serde_json::Value;

/// Rueda "de mentira" para los deslizamientos de la pantalla
pub const STRIP: u8 = 4;

/// Tiempo para deslizar tras tocar, por defecto (ajustable en el panel)
pub const WINDOW_MS: u32 = 1000;
pub const WINDOW_MIN: u32 = 500;
pub const WINDOW_MAX: u32 = 1500;

static LAST_TOUCH: Mutex<Option<(usize, Instant)>> = Mutex::new(None);

/// Apunta que la pantalla se ha tocado encima de la rueda `encoder`
pub fn mark_touch(encoder: usize) {
    *LAST_TOUCH.lock().unwrap() = Some((encoder, Instant::now()));
}

/// Si la pulsación de la rueda `encoder` que acaba de llegar es un toque de
/// la pantalla (sin olvidarlo)
fn is_touch(encoder: usize) -> bool {
    LAST_TOUCH.lock().unwrap().is_some_and(|(e, t)| e == encoder && t.elapsed() < Duration::from_millis(300))
}

/// Si la última pulsación de la rueda `encoder` fue un toque de la pantalla
/// (y se olvida)
pub fn take_touch(encoder: usize) -> bool {
    let mut last = LAST_TOUCH.lock().unwrap();
    let hit = last.is_some_and(|(e, t)| e == encoder && t.elapsed() < Duration::from_millis(500));
    if hit {
        *last = None;
    }
    hit
}

// ------------------------------------------------------------- deslizar --

/// Qué hace deslizar en una rueda (ajustes de su acción)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Swipe {
    /// Lo mismo que girarla
    Rotate,
    /// Pista anterior / siguiente
    Track,
    /// Volumen general − / +
    Volume,
}

/// Lo elegido en una rueda: qué hace y si va al revés
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwipeCfg {
    pub what: Swipe,
    pub invert: bool,
}

impl SwipeCfg {
    /// Lee los ajustes de la acción (`swipe`, `swipe_action`, `swipe_invert`);
    /// `None` si deslizar no hace nada
    pub fn from_settings(v: &Value) -> Option<Self> {
        let what = match v["swipe"].as_str()? {
            "same" => Swipe::Rotate,
            "other" => match v["swipe_action"].as_str().unwrap_or("track") {
                "volume" => Swipe::Volume,
                _ => Swipe::Track,
            },
            _ => return None,
        };
        Some(Self { what, invert: v["swipe_invert"].as_bool().unwrap_or(false) })
    }

    /// Pasos a aplicar por un deslizamiento (`dir`: +1 derecha, −1 izquierda)
    pub fn ticks(&self, dir: i8) -> i16 {
        let d = dir.signum() as i16;
        if self.invert { -d } else { d }
    }
}

struct Knob {
    device: String,
    encoder: u8,
    cfg: SwipeCfg,
}

/// Ruedas de Prisma con el deslizamiento activado, por contexto
static KNOBS: LazyLock<Mutex<HashMap<String, Knob>>> = LazyLock::new(Default::default);

/// Una rueda de Prisma aparece (o cambian sus ajustes)
pub fn register(context: &str, device: &str, encoder: u8, settings: &Value) {
    let mut knobs = KNOBS.lock().unwrap();
    match SwipeCfg::from_settings(settings) {
        Some(cfg) => {
            knobs.insert(context.into(), Knob { device: device.into(), encoder, cfg });
        }
        None => {
            knobs.remove(context);
        }
    }
}

/// Cambian los ajustes de una acción: solo cuenta si es una rueda ya apuntada
/// (el evento de ajustes no dice si es rueda o tecla)
pub fn settings_changed(context: &str, device: &str, encoder: u8, settings: &Value) {
    if KNOBS.lock().unwrap().contains_key(context) || WHEELS.lock().unwrap().contains(context) {
        register(context, device, encoder, settings);
    }
}

/// Ruedas de Prisma visibles (aunque no tengan el deslizamiento activado),
/// para saber a qué contextos hacer caso cuando cambian sus ajustes
static WHEELS: LazyLock<Mutex<std::collections::HashSet<String>>> = LazyLock::new(Default::default);

pub fn appear(context: &str, device: &str, encoder: u8, settings: &Value) {
    WHEELS.lock().unwrap().insert(context.into());
    register(context, device, encoder, settings);
}

pub fn disappear(context: &str) {
    WHEELS.lock().unwrap().remove(context);
    KNOBS.lock().unwrap().remove(context);
}

fn cfg_of(device: &str, encoder: u8) -> Option<SwipeCfg> {
    KNOBS.lock().unwrap().values().find(|k| k.device == device && k.encoder == encoder).map(|k| k.cfg)
}

/// Un toque esperando por si llega el deslizamiento
#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    pub device: String,
    pub encoder: u8,
    at: Instant,
    seq: u64,
}

static HELD: Mutex<(Option<Held>, u64)> = Mutex::new((None, 0));

/// Llega la pulsación de una rueda. Si es un toque de la pantalla en una rueda
/// con deslizamiento, se queda esperando: devuelve su número (para soltarlo
/// con [`expire`] cuando pase el tiempo) y el toque que estuviera esperando
/// antes, que hay que mandar ya.
pub fn hold(device: &str, encoder: u8) -> Option<(u64, Option<Held>)> {
    if !is_touch(encoder as usize) || cfg_of(device, encoder).is_none() {
        return None;
    }
    let mut h = HELD.lock().unwrap();
    h.1 += 1;
    let seq = h.1;
    let prev = h.0.replace(Held { device: device.into(), encoder, at: Instant::now(), seq });
    Some((seq, prev))
}

/// Pasó el tiempo: si el toque `seq` sigue esperando, se suelta (hay que
/// mandarlo como pulsación normal)
pub fn expire(seq: u64) -> Option<Held> {
    let mut h = HELD.lock().unwrap();
    if h.0.as_ref().is_some_and(|x| x.seq == seq) { h.0.take() } else { None }
}

/// Llega un deslizamiento: si hay un toque esperando en ese deck (dentro de
/// `window`), es para su rueda
pub fn take_swipe(device: &str, window: Duration) -> Option<(u8, SwipeCfg)> {
    let held = {
        let mut h = HELD.lock().unwrap();
        match &h.0 {
            Some(x) if x.device == device && x.at.elapsed() <= window => h.0.take()?,
            _ => return None,
        }
    };
    cfg_of(device, held.encoder).map(|c| (held.encoder, c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ajustes_de_deslizar() {
        assert_eq!(SwipeCfg::from_settings(&json!({})), None);
        assert_eq!(SwipeCfg::from_settings(&json!({"swipe": "off"})), None);
        let same = SwipeCfg::from_settings(&json!({"swipe": "same"})).unwrap();
        assert_eq!((same.what, same.ticks(1), same.ticks(-1)), (Swipe::Rotate, 1, -1));
        let vol = SwipeCfg::from_settings(&json!({"swipe": "other", "swipe_action": "volume", "swipe_invert": true})).unwrap();
        assert_eq!((vol.what, vol.ticks(1)), (Swipe::Volume, -1));
        assert_eq!(SwipeCfg::from_settings(&json!({"swipe": "other"})).unwrap().what, Swipe::Track);
    }

    /// Toque o pulsador; tocar y deslizar: el toque espera, y el deslizamiento
    /// se lo lleva (en una sola prueba: comparten el último toque)
    #[test]
    fn toques_y_deslizar() {
        mark_touch(3);
        assert!(!take_touch(1));
        assert!(take_touch(3));
        assert!(!take_touch(3));
        let w = Duration::from_millis(WINDOW_MS as u64);
        appear("c1", "d", 1, &json!({"swipe": "same"}));
        appear("c2", "d", 2, &json!({}));
        // Rueda sin deslizar: el toque va directo
        mark_touch(2);
        assert!(hold("d", 2).is_none());
        // Pulsar el knob (no la pantalla): directo
        assert!(hold("d", 1).is_none());
        // Toque y deslizamiento
        mark_touch(1);
        let (seq, prev) = hold("d", 1).unwrap();
        assert!(prev.is_none());
        assert_eq!(take_swipe("x", w), None);
        assert_eq!(take_swipe("d", w).map(|x| x.0), Some(1));
        assert_eq!(expire(seq), None);
        // Toque solo: se suelta al pasar el tiempo
        mark_touch(1);
        let (seq, _) = hold("d", 1).unwrap();
        assert_eq!(expire(seq).map(|h| h.encoder), Some(1));
        assert_eq!(take_swipe("d", w), None);
        // Cambian los ajustes: deja de esperar
        settings_changed("c1", "d", 1, &json!({"swipe": "off"}));
        mark_touch(1);
        assert!(hold("d", 1).is_none());
        settings_changed("c1", "d", 1, &json!({"swipe": "other"}));
        mark_touch(1);
        assert!(hold("d", 1).is_some());
        disappear("c1");
        disappear("c2");
    }
}
