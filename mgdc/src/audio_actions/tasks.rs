//! Tareas en segundo plano: repintan lo que cambia por fuera (el volumen, la
//! canción) y mueven el reproductor (título que se desliza, ecualizador, GIF)

use std::{collections::HashMap, time::Duration};

use super::{
    Kind,
    render::{dial_moving, dial_song, render_dial_frame, render_key},
    settings::ActSettings,
    state::{INSTANCES, LEVELS, level_of, now_playing, now_playing_cached, send_bg},
    worker::{MEDIA_THREAD, run_blocking, run_on},
};
use crate::audio::{self, Level, NowPlaying};

/// Repinta lo que haya cambiado por fuera (otro programa, el teclado, la canción…)
pub async fn poll_task() {
    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let items: Vec<(String, Kind, ActSettings)> = INSTANCES
            .lock()
            .await
            .iter()
            .filter(|(_, i)| !i.animating && !matches!(i.kind, Kind::Stop | Kind::Next | Kind::Prev))
            // Reproducir/Pausa con lo que suena lo pinta player_task; aquí solo se
            // consulta lo que suena para que lo tenga al día
            .map(|(c, i)| (c.clone(), i.kind, i.settings.clone()))
            .collect();
        if items.is_empty() {
            continue;
        }
        // Una sola consulta por destino y una sola de "lo que suena"
        let mut levels: HashMap<String, Option<Level>> = HashMap::new();
        for (_, k, s) in &items {
            if k.uses_level() && !levels.contains_key(&s.target) {
                levels.insert(s.target.clone(), level_of(&s.target).await);
            }
        }
        let needs_np = items.iter().any(|(_, k, s)| *k == Kind::PlayPause || (*k == Kind::Dial && s.show_title));
        let np = if needs_np { now_playing().await } else { None };

        for (context, kind, settings) in items {
            if settings.shows_player(kind) {
                continue;
            }
            let level = levels.get(&settings.target).copied().flatten();
            // La rueda que se mueve la pinta player_task
            if kind == Kind::Dial && dial_moving(&settings, level, np.as_ref()) {
                continue;
            }
            let svg = render_key(kind, &settings, level, np.as_ref(), None);
            let changed = match INSTANCES.lock().await.get_mut(&context) {
                Some(i) if !i.animating && i.last != svg => {
                    i.last = svg.clone();
                    true
                }
                _ => false,
            };
            if changed {
                send_bg(&context, kind, &svg).await;
            }
        }
    }
}

/// Pinta las teclas Reproducir/Pausa con lo que suena: 8 veces por segundo
/// mientras algo se mueve (4 en el MSD-ONE); si no cambia nada, no se envía.
pub async fn player_task() {
    use crate::player::{self, Bar, EqMode, Frame};
    let mut tick: u64 = 0;
    loop {
        tokio::time::sleep(Duration::from_millis(125)).await;
        tick += 1;
        let all: Vec<(String, Kind, ActSettings)> = INSTANCES
            .lock()
            .await
            .iter()
            .filter(|(_, i)| !i.animating && (i.settings.shows_player(i.kind) || i.kind == Kind::Dial))
            .map(|(c, i)| (c.clone(), i.kind, i.settings.clone()))
            .collect();
        if all.is_empty() {
            continue;
        }
        let np = now_playing_cached().await;
        let dials: Vec<(String, ActSettings)> =
            all.iter().filter(|(_, k, _)| *k == Kind::Dial).map(|(c, _, s)| (c.clone(), s.clone())).collect();
        let items: Vec<(String, ActSettings)> =
            all.into_iter().filter(|(_, k, _)| *k != Kind::Dial).map(|(c, _, s)| (c, s)).collect();
        player_dials(dials, np.as_ref(), tick).await;
        if items.is_empty() {
            continue;
        }
        let playing = np.as_ref().is_some_and(|n| n.playing);
        let eq_on = |s: &ActSettings, m: EqMode| {
            playing && Bar::from_id(&s.bar).for_deck(s.small).uses_eq() && EqMode::from_id(&s.eq_mode) == m
        };
        // Volumen real del programa que suena, y frecuencias, solo si alguien los usa
        let level = if items.iter().any(|(_, s)| eq_on(s, EqMode::Level)) {
            let process = np.as_ref().map(|n| n.process.clone()).unwrap_or_default();
            run_blocking(move || audio::peak(&process)).await.flatten().unwrap_or(0.0)
        } else {
            0.0
        };

        for (context, s) in items {
            // El MSD-ONE aguanta menos cambios: la mitad de fotogramas
            if s.small && tick % 2 == 1 {
                continue;
            }
            let look = s.look();
            let song = np.as_ref().map(|n| format!("{}\u{1}{}", n.title, n.artist)).unwrap_or_default();
            let (t, eq) = {
                let mut map = INSTANCES.lock().await;
                let Some(i) = map.get_mut(&context) else { continue };
                if i.song != song {
                    i.song = song;
                    i.since = std::time::Instant::now();
                }
                if Bar::from_id(&s.bar).for_deck(s.small).uses_eq() {
                    i.eq = if playing {
                        player::next_eq(i.eq, EqMode::from_id(&s.eq_mode), level, tick)
                    } else {
                        [0.0; player::EQ_BARS]
                    };
                }
                (i.since.elapsed().as_secs_f32(), i.eq)
            };
            let gif = if look.bar.for_deck(s.small) == Bar::Gif && playing {
                let (id, t) = (s.gif.clone(), t);
                run_on(&MEDIA_THREAD, move || player::gif_frame(&id, t)).await.flatten()
            } else {
                None
            };
            let moving = player::moving(np.as_ref(), &look);
            let frame = if moving { Frame { t, eq, gif: gif.as_deref() } } else { Frame::still() };
            let svg = player::render(np.as_ref(), &look, &frame, false);
            let changed = match INSTANCES.lock().await.get_mut(&context) {
                Some(i) if !i.animating && i.last != svg => {
                    i.last = svg.clone();
                    true
                }
                _ => false,
            };
            if changed {
                send_bg(&context, Kind::PlayPause, &svg).await;
            }
        }
    }
}

/// Fotograma de las ruedas que se mueven (ecualizador o título que se desliza)
pub(super) async fn player_dials(dials: Vec<(String, ActSettings)>, np: Option<&NowPlaying>, tick: u64) {
    use crate::player::{self, DialStyle, EqMode};
    let playing = np.is_some_and(|n| n.playing);
    for (context, s) in dials {
        let level = LEVELS.lock().await.get(&s.target).copied().flatten();
        if !dial_moving(&s, level, np) {
            continue;
        }
        // Volumen real: del sonido general o del programa de la rueda
        let peak = if DialStyle::from_id(&s.dial_style).has_eq() && EqMode::from_id(&s.eq_mode) == EqMode::Level {
            let who = if audio::is_general(&s.target) { String::new() } else { s.target.trim().to_lowercase() };
            run_blocking(move || audio::peak(&who)).await.flatten().unwrap_or(0.0)
        } else {
            0.0
        };
        let (title, artist) = dial_song(&s, level, np);
        let song = format!("{title}\u{1}{artist}");
        let (t, eq) = {
            let mut map = INSTANCES.lock().await;
            let Some(i) = map.get_mut(&context) else { continue };
            if i.song != song {
                i.song = song;
                i.since = std::time::Instant::now();
            }
            i.eq = if playing { player::next_eq(i.eq, EqMode::from_id(&s.eq_mode), peak, tick) } else { [0.0; player::EQ_BARS] };
            (i.since.elapsed().as_secs_f32(), i.eq)
        };
        let svg = render_dial_frame(&s, level, np, None, &player::Frame { t, eq, gif: None });
        let changed = match INSTANCES.lock().await.get_mut(&context) {
            Some(i) if !i.animating && i.last != svg => {
                i.last = svg.clone();
                true
            }
            _ => false,
        };
        if changed {
            send_bg(&context, Kind::Dial, &svg).await;
        }
    }
}
