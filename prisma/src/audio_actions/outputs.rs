//! Elegir la salida de audio desde la rueda de volumen (altavoces,
//! auriculares…).
//!
//! Doble click entra en «elegir salida»: se ve la salida actual con su dibujo
//! y un recuadro. Girar cambia de salida al momento (el sonido sale ya por la
//! nueva); un click termina, y si no se toca en 10 s también. Mientras tanto
//! lo demás del deck funciona normal. Las salidas se leen al entrar
//! (así salen las que haya conectadas en ese momento).
//!
//! Fuera de ese modo, de fondo de la rueda se ve tenue el dibujo de la salida
//! que suena.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex as StdMutex},
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

use super::{
    Kind,
    state::{INSTANCES, refresh, send_bg, set_animating},
    worker::run_blocking,
};
use crate::{
    audio::{self, Output, OutputKind},
    i18n::{t, tf},
    svg::esc,
};

/// Sin tocar la rueda este tiempo, se queda la salida que se ve
const IDLE: Duration = Duration::from_secs(10);
/// Cada cuánto se mira qué salida suena (para el dibujo de fondo)
const KIND_EVERY: Duration = Duration::from_secs(5);

const FRAME: &str = "#4aa3ff";
const DIM: &str = "#24405c";
const DIM_NORMAL: &str = "#16283a";

struct Choosing {
    list: Vec<Output>,
    index: usize,
    touched: Instant,
}

static CHOOSING: LazyLock<Mutex<HashMap<String, Choosing>>> = LazyLock::new(Default::default);

/// Qué salida suena ahora (para el fondo) y cuándo se miró
static CURRENT: StdMutex<(Option<OutputKind>, Option<Instant>)> = StdMutex::new((None, None));

/// Si la rueda está eligiendo salida
pub(super) async fn is_choosing(context: &str) -> bool {
    CHOOSING.lock().await.contains_key(context)
}

/// Doble click: entra a elegir salida
pub(super) async fn start(context: String) {
    let list = run_blocking(audio::outputs).await.unwrap_or_default();
    if list.is_empty() {
        log::warn!("Audio: no hay salidas de audio que elegir");
        return;
    }
    let index = list.iter().position(|o| o.default).unwrap_or(0);
    log::info!("Audio: elegir salida ({} salidas)", list.len());
    set_animating(&context, true).await;
    send_bg(&context, Kind::Dial, &render(&list, index)).await;
    CHOOSING.lock().await.insert(context.clone(), Choosing { list, index, touched: Instant::now() });
    // Si no se toca en un rato, se queda la que se vea
    tokio::spawn(async move {
        loop {
            let wait = match CHOOSING.lock().await.get(&context) {
                Some(c) => IDLE.saturating_sub(c.touched.elapsed()),
                None => return,
            };
            if wait.is_zero() {
                confirm(&context).await;
                return;
            }
            tokio::time::sleep(wait).await;
        }
    });
}

/// Girar mientras se elige: cambia a la salida anterior o siguiente al
/// momento (devuelve false si no se estaba eligiendo)
pub(super) async fn rotate(context: &str, ticks: i16) -> bool {
    let (svg, chosen) = {
        let mut all = CHOOSING.lock().await;
        let Some(c) = all.get_mut(context) else { return false };
        let n = c.list.len() as i64;
        c.index = (c.index as i64 + ticks as i64).rem_euclid(n) as usize;
        c.touched = Instant::now();
        let index = c.index;
        for (i, o) in c.list.iter_mut().enumerate() {
            o.default = i == index;
        }
        (render(&c.list, c.index), c.list[c.index].clone())
    };
    send_bg(context, Kind::Dial, &svg).await;
    // Todo el audio va por un mismo hilo: los cambios se aplican en orden
    let id = chosen.id.clone();
    let r = run_blocking(move || {
        let r = audio::set_default_output(&id);
        // El volumen que se vea luego es el de la nueva salida
        audio::forget_devices();
        r
    })
    .await;
    log::info!("Audio: salida {} → {r:?}", chosen.name);
    *CURRENT.lock().unwrap() = (Some(chosen.kind), Some(Instant::now()));
    true
}

/// Click (o 10 s quieta): termina de elegir
pub(super) async fn confirm(context: &str) -> bool {
    if CHOOSING.lock().await.remove(context).is_none() {
        return false;
    }
    set_animating(context, false).await;
    // Todas las ruedas y teclas de volumen, con la nueva salida
    let contexts: Vec<String> = INSTANCES.lock().await.keys().cloned().collect();
    for ctx in contexts {
        refresh(&ctx).await;
    }
    true
}

/// La rueda ya no se ve: deja de elegir sin cambiar nada
pub(super) async fn forget(context: &str) {
    CHOOSING.lock().await.remove(context);
}

/// Mira qué salida suena, si hace rato que no se miraba
pub(super) async fn update_current() {
    let due = CURRENT.lock().unwrap().1.is_none_or(|at| at.elapsed() >= KIND_EVERY);
    if due {
        let kind = run_blocking(audio::default_output_kind).await.flatten();
        *CURRENT.lock().unwrap() = (kind, Some(Instant::now()));
    }
}

/// Fondo tenue de la rueda de volumen: la salida que suena
pub(super) fn background() -> String {
    CURRENT.lock().unwrap().0.map(|k| icon(k, DIM_NORMAL)).unwrap_or_default()
}

/// Dibujo de cada tipo de salida, centrado en la pantalla de la rueda
fn icon(kind: OutputKind, color: &str) -> String {
    let body = match kind {
        OutputKind::Speakers => r##"<path d="M70 38 L88 38 L110 20 L110 80 L88 62 L70 62 Z"/><path d="M122 36 Q134 50 122 64"/><path d="M132 28 Q150 50 132 72"/>"##,
        OutputKind::Headphones => r##"<path d="M68 66 L68 50 Q68 18 100 18 Q132 18 132 50 L132 66"/><rect x="60" y="54" width="16" height="26" rx="5"/><rect x="124" y="54" width="16" height="26" rx="5"/>"##,
        OutputKind::Headset => r##"<path d="M68 62 L68 50 Q68 20 100 20 Q132 20 132 50 L132 62"/><rect x="60" y="50" width="16" height="24" rx="5"/><rect x="124" y="50" width="16" height="24" rx="5"/><path d="M68 74 Q72 88 94 88"/>"##,
        OutputKind::Display => r##"<rect x="58" y="20" width="84" height="50" rx="4"/><path d="M88 82 L112 82 M100 70 L100 82"/>"##,
        OutputKind::Digital => r##"<rect x="76" y="30" width="48" height="40" rx="6"/><path d="M100 70 L100 86 M88 30 L88 18 M112 30 L112 18"/>"##,
    };
    format!(r##"<g fill="none" stroke="{color}" stroke-width="5" stroke-linecap="round" stroke-linejoin="round">{body}</g>"##)
}

/// Pantalla de la rueda mientras se elige
fn render(list: &[Output], index: usize) -> String {
    let o = &list[index];
    let name = crate::player::fit(&o.name, 17.0, true, 140.0);
    let foot = t("audio.output_done");
    let arrows = if list.len() > 1 {
        r##"<text x="14" y="62" font-family="sans-serif" font-size="22" fill="#9a9a9a">‹</text><text x="176" y="62" font-family="sans-serif" font-size="22" fill="#9a9a9a">›</text>"##
    } else {
        ""
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100"><rect width="200" height="100" fill="#000"/>{icon}<rect x="2" y="2" width="196" height="96" rx="4" fill="none" stroke="{FRAME}" stroke-width="3"/><text x="100" y="22" font-family="sans-serif" font-size="12" fill="{FRAME}" text-anchor="middle">{head}</text><text x="100" y="58" font-family="sans-serif" font-size="17" font-weight="bold" fill="#fff" text-anchor="middle">{name}</text>{arrows}<text x="100" y="88" font-family="sans-serif" font-size="11" fill="#9a9a9a" text-anchor="middle">{foot}</text></svg>"##,
        icon = icon(o.kind, DIM),
        head = esc(&tf("audio.output_head", &[&(index + 1), &list.len()])),
        name = esc(&name),
        foot = esc(foot),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(name: &str, kind: OutputKind, default: bool) -> Output {
        Output { id: name.into(), name: name.into(), kind, default }
    }

    #[test]
    fn elegir_salida() {
        let list = vec![out("Altavoces", OutputKind::Speakers, true), out("Auriculares & co", OutputKind::Headphones, false)];
        let a = render(&list, 0);
        let b = render(&list, 1);
        for s in [&a, &b] {
            assert!(s.starts_with("<svg") && s.ends_with("</svg>"));
        }
        assert!(a.contains("1 / 2") && a.contains(t("audio.output_done")));
        assert!(b.contains("2 / 2") && b.contains("&amp;"));
        for k in [OutputKind::Speakers, OutputKind::Headphones, OutputKind::Headset, OutputKind::Display, OutputKind::Digital] {
            assert!(render(&[out("x", k, false)], 0).contains("<g fill=\"none\""));
        }
        // Una sola salida: sin flechas
        assert!(!render(&list[..1], 0).contains('‹'));
    }

    #[tokio::test]
    async fn girar_sin_elegir() {
        assert!(!rotate("nadie", 1).await);
        assert!(!confirm("nadie").await);
    }
}
