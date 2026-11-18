//! Lo que hacen las acciones de audio al pulsarlas y al girar la rueda

use std::{collections::HashMap, sync::LazyLock, time::Duration};

use tokio::sync::Mutex;

use super::{
    Kind,
    render::{dial_moving, render_dial, render_dial_frame, render_key},
    settings::ActSettings,
    state::{INSTANCES, expect_playing, now_playing, now_playing_cached, refresh, remember_level, send_bg, set_animating},
    worker::{KEY_THREAD, run_blocking, run_on},
};
use crate::audio::{self, Level, MediaKey, NowPlaying};

/// Animación de una tecla al pulsarla (4 fotogramas) y luego su estado real
pub(super) fn animate_key(context: String, kind: Kind, settings: ActSettings, level: Option<Level>, np: Option<NowPlaying>) {
    tokio::spawn(async move {
        set_animating(&context, true).await;
        for f in 1..4u8 {
            let svg = render_key(kind, &settings, level, np.as_ref(), Some(f));
            send_bg(&context, kind, &svg).await;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        set_animating(&context, false).await;
        refresh(&context).await;
    });
}

// ----------------------------------------------------------------- eventos --

/// Pulsar una tecla de audio
pub(super) async fn press(kind: Kind, context: String, s: ActSettings) {
    // La tecla responde al momento, antes de hablar con Windows
    set_animating(&context, true).await;
    let np0 = now_playing_cached().await;
    send_bg(&context, kind, &render_key(kind, &s, None, np0.as_ref(), Some(0))).await;
    let step = s.step.clamp(1, 50) as i32;
    let target = s.target.clone();
    let (level, np) = match kind {
        Kind::PlayPause | Kind::Stop | Kind::Next | Kind::Prev => {
            let key = match kind {
                Kind::PlayPause => MediaKey::PlayPause,
                Kind::Stop => MediaKey::Stop,
                Kind::Next => MediaKey::Next,
                _ => MediaKey::Prev,
            };
            // Va por su propio hilo: nada de lo que se esté consultando la retrasa
            match run_on(&KEY_THREAD, move || crate::media::send(key)).await {
                Some(r) => log::info!("Audio: {key:?}: {r}"),
                None => log::warn!("Audio: {key:?} no terminó"),
            }
            if matches!(kind, Kind::PlayPause | Kind::Stop) {
                // La tecla cambia al momento (sin esperar a que Windows se entere);
                // luego se comprueba lo que de verdad pasó
                expect_playing(kind == Kind::PlayPause && !np0.as_ref().is_some_and(|n| n.playing)).await;
                let context = context.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    now_playing().await;
                    refresh(&context).await;
                });
            }
            (None, np0)
        }
        Kind::VolUp => (run_blocking(move || audio::change_volume(&target, step)).await.flatten(), None),
        Kind::VolDown => (run_blocking(move || audio::change_volume(&target, -step)).await.flatten(), None),
        Kind::Mute => (run_blocking(move || audio::toggle_mute(&target)).await.flatten(), None),
        Kind::Dial => {
            set_animating(&context, false).await;
            return;
        }
    };
    if kind.uses_level() {
        log::info!("Audio: {} ahora {:?}", audio::display_name(&s.target), level);
    }
    animate_key(context, kind, s, level, np);
}

/// Giros de rueda pendientes de aplicar, por rueda
pub(super) static PENDING: LazyLock<Mutex<HashMap<String, i32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Girar una rueda de volumen. Si llegan giros mientras se aplica uno, se
/// juntan y se aplican de una vez: así la rueda no se queda atrás.
pub(super) async fn rotate(context: String, s: ActSettings, ticks: i16) {
    {
        let mut p = PENDING.lock().await;
        if let Some(n) = p.get_mut(&context) {
            *n += ticks as i32;
            return; // ya hay quien los aplica
        }
        p.insert(context.clone(), ticks as i32);
    }
    set_animating(&context, true).await;
    loop {
        let ticks = {
            let mut p = PENDING.lock().await;
            let n = p.get(&context).copied().unwrap_or(0);
            if n == 0 {
                p.remove(&context);
                break;
            }
            p.insert(context.clone(), 0);
            n
        };
        let delta = ticks * s.step.clamp(1, 50) as i32;
        let target = s.target.clone();
        let after = run_blocking(move || audio::change_volume(&target, delta)).await.flatten();
        remember_level(&s.target, after).await;
        log::info!("Audio: {} {delta:+} → {:?}", audio::display_name(&s.target), after);
        let np = if s.show_title { now_playing_cached().await } else { None };
        // Mismo punto del deslizamiento y del ecualizador: girar no los reinicia
        let (t, eq) = match INSTANCES.lock().await.get(&context) {
            Some(i) => (i.since.elapsed().as_secs_f32(), i.eq),
            None => (0.0, [0.0; crate::player::EQ_BARS]),
        };
        let frame = if dial_moving(&s, after, np.as_ref()) { crate::player::Frame { t, eq, gif: None } } else { crate::player::Frame::still() };
        send_bg(&context, Kind::Dial, &render_dial_frame(&s, after, np.as_ref(), None, &frame)).await;
    }
    set_animating(&context, false).await;
    if let Some(i) = INSTANCES.lock().await.get_mut(&context) {
        i.last.clear();
    }
}

/// Pulsar la rueda: silenciar o quitar el silencio
pub(super) async fn dial_mute(context: String, s: ActSettings) {
    let target = s.target.clone();
    let after = run_blocking(move || audio::toggle_mute(&target)).await.flatten();
    remember_level(&s.target, after).await;
    log::info!("Audio: silencio de {} → {:?}", audio::display_name(&s.target), after);
    if let Some(l) = after {
        let np = if s.show_title { now_playing_cached().await } else { None };
        send_bg(&context, Kind::Dial, &render_dial(&s, Some(l), np.as_ref(), None)).await;
    }
}
