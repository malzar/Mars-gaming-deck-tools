//! La rueda de perfiles del MSD-PRO: girar recorre el nivel y pulsar (o tocar
//! la pantalla encima) cambia, con click y doble click configurables.
//!
//! Dos formas (ajuste de la rueda):
//! - "press": girar recorre la lista y pulsar la rueda (o tocar la pantalla) cambia
//! - "direct": girar cambia directamente (al dejar de girar un momento)

use std::{
    collections::HashMap,
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use openaction::OutboundEventManager;
use tokio::sync::Mutex;

use super::{
    draw::{BASES, redraw, redraw_device, render},
    nav::{choose, go, step, view},
    slots::virtual_count,
};
use crate::clicks::DOUBLE;

pub(super) struct Dial {
    pub(super) device: String,
    /// Rueda del deck (0 = la de la izquierda)
    pub(super) position: u8,
    pub(super) direct: bool,
    pub(super) presses: Presses,
    /// Pulsación esperando por si llega la segunda (doble click): cuándo y si
    /// fue la pantalla
    pub(super) pending: Option<(Instant, bool, u64)>,
    /// Lo último que se dibujó (para mover el icono si es un GIF sin releer nada)
    pub(super) shown: Option<Shown>,
}

#[derive(Clone)]
pub(super) struct Shown {
    pub(super) target: Option<String>,
    pub(super) at: usize,
    pub(super) total: usize,
}
pub(super) static WHEELS: LazyLock<Mutex<HashMap<String, Dial>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Para "direct": solo cambia la última vuelta de rueda (las anteriores se anulan)
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn direct_of(settings: &serde_json::Value) -> bool {
    settings.get("mode").and_then(|m| m.as_str()) == Some("direct")
}

/// Qué hace una pulsación (de la rueda o de la pantalla)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Press {
    /// Cambia al perfil elegido
    Go,
    /// Nada
    Nothing,
    /// Cambia al primer subperfil del elegido (y la rueda pasa a sus
    /// subperfiles); si no tiene, nada
    First,
}

/// Lo que hacen click y doble click, en la rueda y en la pantalla
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Presses {
    pub click: Press,
    pub double: Press,
    pub tap: Press,
    pub double_tap: Press,
}

fn press_of(settings: &serde_json::Value, key: &str, default: Press) -> Press {
    match settings.get(key).and_then(|m| m.as_str()) {
        Some("go") => Press::Go,
        Some("none") => Press::Nothing,
        Some("first") => Press::First,
        _ => default,
    }
}

fn presses_of(settings: &serde_json::Value) -> Presses {
    Presses {
        click: press_of(settings, "click", Press::Go),
        double: press_of(settings, "double", Press::Nothing),
        tap: press_of(settings, "tap", Press::Go),
        double_tap: press_of(settings, "doubleTap", Press::Nothing),
    }
}

pub async fn appear(context: &str, device: &str, position: u8, settings: &serde_json::Value, outbound: &mut OutboundEventManager) {
    // Primera rueda de perfiles en un deck sin teclas virtuales: se crean ya
    // (una vez por sesión, para no repetirlo si no se puede)
    if virtual_count(device) == 0 {
        static ASKED: LazyLock<std::sync::Mutex<Vec<String>>> = LazyLock::new(|| std::sync::Mutex::new(vec![]));
        let first = { let mut a = ASKED.lock().unwrap(); let f = !a.iter().any(|d| d == device); if f { a.push(device.to_string()); } f };
        if first {
            let d = device.to_string();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(500)).await;
                crate::device::reload_profiles(&d, || ()).await;
            });
        }
    }
    WHEELS.lock().await.insert(
        context.to_string(),
        Dial { device: device.to_string(), position, direct: direct_of(settings), presses: presses_of(settings), pending: None, shown: None },
    );
    let svg = render(context).await;
    crate::canvas::send_with(outbound, context, true, &svg).await;
}

pub async fn disappear(context: &str) {
    if let Some(w) = WHEELS.lock().await.remove(context) {
        BASES.lock().await.remove(&(w.device, w.position));
    }
}

pub async fn settings_changed(context: &str, settings: &serde_json::Value) {
    if let Some(w) = WHEELS.lock().await.get_mut(context) {
        w.direct = direct_of(settings);
        w.presses = presses_of(settings);
    }
    redraw(context).await;
}

pub async fn rotate(context: &str, ticks: i16) {
    let Some((device, direct)) = WHEELS.lock().await.get(context).map(|w| (w.device.clone(), w.direct)) else { return };
    if step(&device, ticks).await.is_none() {
        redraw(context).await;
        return;
    }
    redraw_device(&device).await;
    if direct {
        // Cambia al dejar de girar un momento, para no pasar por todos los perfiles
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(450)).await;
            if GENERATION.load(Ordering::SeqCst) == generation {
                choose(&device).await;
            }
        });
    }
}

/// Pulsar la rueda o tocar la pantalla encima: click o doble click, cada uno
/// con lo que diga su ajuste. Si el doble no hace nada, el click va al momento.
pub async fn press(context: &str) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let decided = {
        let mut wheels = WHEELS.lock().await;
        let Some(w) = wheels.get_mut(context) else { return };
        let touch = crate::gestures::take_touch(w.position as usize);
        let (single, double) = if touch { (w.presses.tap, w.presses.double_tap) } else { (w.presses.click, w.presses.double) };
        match w.pending.take() {
            // Segunda pulsación a tiempo: doble click
            Some((t, was_touch, _)) if t.elapsed() < DOUBLE && was_touch == touch => Some((w.device.clone(), double)),
            _ if double == Press::Nothing => Some((w.device.clone(), single)),
            // Espera por si llega la segunda
            _ => {
                let n = NEXT.fetch_add(1, Ordering::SeqCst);
                w.pending = Some((Instant::now(), touch, n));
                let (context, device) = (context.to_string(), w.device.clone());
                tokio::spawn(async move {
                    tokio::time::sleep(DOUBLE).await;
                    let still = {
                        let mut wheels = WHEELS.lock().await;
                        match wheels.get_mut(&context) {
                            Some(w) if w.pending.is_some_and(|(_, _, m)| m == n) => {
                                w.pending = None;
                                true
                            }
                            _ => false,
                        }
                    };
                    if still {
                        act(&device, single).await;
                    }
                });
                None
            }
        }
    };
    if let Some((device, what)) = decided {
        act(&device, what).await;
    }
}

/// Deslizar por la pantalla: como girar la rueda de perfiles del deck
pub async fn swipe(device: &str, ticks: i16) {
    let context = WHEELS.lock().await.iter().find(|(_, w)| w.device == device).map(|(c, _)| c.clone());
    match context {
        Some(c) => rotate(&c, ticks).await,
        None => log::debug!("Pantalla: deslizar en {device} sin rueda de perfiles"),
    }
}

async fn act(device: &str, what: Press) {
    match what {
        Press::Nothing => {}
        Press::Go => choose(device).await,
        Press::First => {
            let v = view(device).await;
            let Some(sel) = v.selected.clone() else { return };
            // Sin subperfiles no hace nada: lo que haga cada pulsación lo elige el usuario
            if let Some(first) = v.kids(&sel).first() {
                go(device, first, false).await;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ajustes_de_pulsar() {
        let p = presses_of(&serde_json::json!({}));
        assert_eq!(p, Presses { click: Press::Go, double: Press::Nothing, tap: Press::Go, double_tap: Press::Nothing });
        let p = presses_of(&serde_json::json!({ "click": "none", "double": "first", "tap": "first", "doubleTap": "go" }));
        assert_eq!(p, Presses { click: Press::Nothing, double: Press::First, tap: Press::First, double_tap: Press::Go });
    }
}
