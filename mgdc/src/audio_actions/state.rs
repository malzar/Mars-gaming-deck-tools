//! Estado de las acciones de audio: lo que hay en cada tecla, el último
//! volumen y lo que suena, y el repintado cuando algo cambia.

use std::{collections::HashMap, sync::LazyLock, time::Duration};

use tokio::sync::Mutex;

use super::{
    Kind,
    render::render_key,
    settings::ActSettings,
    worker::{MEDIA_BUSY, MEDIA_THREAD, run_blocking, run_on},
};
use crate::audio::{self, Level, NowPlaying};

pub(super) struct Instance {
    pub(super) kind: Kind,
    pub(super) settings: ActSettings,
    /// Último dibujo enviado, para no repetir envíos iguales
    pub(super) last: String,
    /// Mientras se anima, el refresco periódico no la toca
    pub(super) animating: bool,
    /// Reproducir/Pausa: cuándo empezó la canción que se ve (para deslizar el
    /// título desde el principio), cuál era y cómo estaban las barras
    pub(super) since: std::time::Instant,
    pub(super) song: String,
    pub(super) eq: [f32; crate::player::EQ_BARS],
}

impl Instance {
    pub(super) fn new(kind: Kind, settings: ActSettings, last: String) -> Self {
        Instance { kind, settings, last, animating: false, since: std::time::Instant::now(), song: String::new(), eq: [0.0; crate::player::EQ_BARS] }
    }
}

pub(super) static INSTANCES: LazyLock<Mutex<HashMap<String, Instance>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Último volumen conocido de cada destino (para dibujar la rueda sin preguntar
/// a Windows en cada fotograma)
pub(super) static LEVELS: LazyLock<Mutex<HashMap<String, Option<Level>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) async fn remember_level(target: &str, level: Option<Level>) {
    LEVELS.lock().await.insert(target.to_string(), level);
}

// ------------------------------------------------------------- estado real --

pub(super) async fn level_of(target: &str) -> Option<Level> {
    let t = target.to_string();
    let l = run_blocking(move || audio::level(&t)).await.flatten();
    remember_level(target, l).await;
    l
}

/// Lo que suena, consultado a Windows (y guardado para las pulsaciones)
pub(super) async fn now_playing() -> Option<NowPlaying> {
    use std::sync::atomic::Ordering;
    // Si la consulta anterior sigue sin contestar, no se encolan más
    if MEDIA_BUSY.swap(true, Ordering::AcqRel) {
        return now_playing_cached().await;
    }
    let started = std::time::Instant::now();
    let mut np = run_on(&MEDIA_THREAD, || {
        let r = audio::now_playing();
        MEDIA_BUSY.store(false, Ordering::Release);
        r
    })
    .await
    .flatten();
    // Recién pulsada la tecla, manda lo que se espera; luego lo que diga Windows
    let expected = *EXPECTED.lock().await;
    if let (Some(n), Some((playing, until))) = (np.as_mut(), expected)
        && std::time::Instant::now() < until
    {
        n.playing = playing;
    }
    let ms = started.elapsed().as_millis();
    if ms > 300 {
        log::warn!("Audio: saber lo que suena ha tardado {ms} ms");
    }
    *NOW_PLAYING.lock().await = np.clone();
    np
}

/// Lo último que se supo de lo que suena, sin esperar a Windows
pub(super) static NOW_PLAYING: LazyLock<Mutex<Option<NowPlaying>>> = LazyLock::new(|| Mutex::new(None));

/// Tras pulsar, lo que se espera (sonando o no) y hasta cuándo se le hace caso
/// por encima de lo que diga Windows, que tarda en enterarse
pub(super) static EXPECTED: LazyLock<Mutex<Option<(bool, std::time::Instant)>>> = LazyLock::new(|| Mutex::new(None));

pub(super) async fn expect_playing(playing: bool) {
    *EXPECTED.lock().await = Some((playing, std::time::Instant::now() + Duration::from_millis(2500)));
    if let Some(n) = NOW_PLAYING.lock().await.as_mut() {
        n.playing = playing;
    }
}

pub(super) async fn now_playing_cached() -> Option<NowPlaying> {
    NOW_PLAYING.lock().await.clone()
}

/// Dibuja una acción con su estado real, solo si ha cambiado. Consulta Windows
/// sin tener el gestor de eventos, así una consulta lenta no para el plugin.
pub(super) async fn refresh(context: &str) {
    let (kind, settings) = match INSTANCES.lock().await.get(context) {
        Some(i) => (i.kind, i.settings.clone()),
        None => return,
    };
    let level = if kind.uses_level() { level_of(&settings.target).await } else { None };
    let np = if kind == Kind::PlayPause || (kind == Kind::Dial && settings.show_title) { now_playing().await } else { None };
    let svg = render_key(kind, &settings, level, np.as_ref(), None);
    let changed = match INSTANCES.lock().await.get_mut(context) {
        Some(i) if i.last != svg => {
            i.last = svg.clone();
            true
        }
        _ => false,
    };
    if changed {
        send_bg(context, kind, &svg).await;
    }
}

// --------------------------------------------------------------- animación --

pub(super) async fn set_animating(context: &str, on: bool) {
    if let Some(i) = INSTANCES.lock().await.get_mut(context) {
        i.animating = on;
        if !on {
            i.last.clear();
        }
    }
}

/// Envía un dibujo desde segundo plano (tecla o rueda según la acción)
pub(super) async fn send_bg(context: &str, kind: Kind, svg: &str) {
    crate::canvas::send(context, kind.on_dial(), svg).await;
}
