//! Dibujo de las teclas y de las pantallas de las ruedas de audio

use super::{Kind, settings::ActSettings};
use crate::{
    audio::{self, Level, NowPlaying},
    svg::{clip, esc},
};

pub(super) const SPEAKER: &str = r##"<path d="M30 58h18l24-20v68l-24-20H30z" fill="#fff"/>"##;

pub(super) fn waves(lit: usize) -> String {
    (0..3)
        .map(|i| {
            let x = 84 + i * 12;
            let h = 16 + i * 10;
            format!(
                r##"<path d="M{x} {y1}q{dx} {h} 0 {h2}" stroke="#fff" stroke-width="6" fill="none" stroke-linecap="round" opacity="{o}"/>"##,
                y1 = 72 - h,
                dx = 10 + i * 4,
                h2 = h * 2,
                o = if i < lit { "1" } else { "0.2" }
            )
        })
        .collect()
}

pub(super) fn key_svg(body: &str, caption: &str) -> String {
    let cap = if caption.is_empty() {
        String::new()
    } else {
        format!(
            r##"<text x="72" y="134" font-family="sans-serif" font-size="18" fill="#fff" text-anchor="middle">{}</text>"##,
            esc(caption)
        )
    };
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 144 144" width="144" height="144"><rect width="144" height="144" fill="#000"/>{body}{cap}</svg>"##)
}

/// Dibujo de una tecla. `frame` = fotograma de la animación al pulsar (0-3).
pub(super) fn render_key(kind: Kind, s: &ActSettings, level: Option<Level>, np: Option<&NowPlaying>, frame: Option<u8>) -> String {
    let f = frame.unwrap_or(3) as usize;
    let caption_level = |l: Option<Level>| match l {
        Some(l) if l.muted => crate::i18n::tf("audio.muted_caption", &[&clip(&audio::display_name(&s.target), 9)]),
        Some(l) => format!("{} {} %", clip(&audio::display_name(&s.target), 9), l.volume),
        None if audio::is_general(&s.target) => crate::i18n::t("audio.master").to_string(),
        None => crate::i18n::tf("audio.closed_caption", &[&clip(&audio::display_name(&s.target), 8)]),
    };
    match kind {
        Kind::PlayPause if s.show_title => {
            let pressed = frame.is_some() && f < 3;
            crate::player::render(np, &s.look(), &crate::player::Frame::still(), pressed)
        }
        Kind::PlayPause => {
            let playing = np.is_some_and(|n| n.playing);
            let title = np.filter(|_| s.show_title).map(|n| n.title.trim().to_string()).unwrap_or_default();
            let (y, scale) = if title.is_empty() { (0, 1.0) } else { (-18, 0.75) };
            let icon = if playing {
                r##"<rect x="46" y="38" width="18" height="68" rx="4" fill="#fff"/><rect x="80" y="38" width="18" height="68" rx="4" fill="#fff"/>"##
            } else {
                r##"<path d="M52 36l50 36-50 36z" fill="#fff"/>"##
            };
            let pulse = if frame.is_some() && f < 3 { r##" opacity="0.6""## } else { "" };
            let body = format!(r#"<g transform="translate(72 {cy}) scale({scale}) translate(-72 -72)"{pulse}>{icon}</g>"#, cy = 72 + y);
            key_svg(&body, &clip(&title, 13))
        }
        Kind::Stop => {
            let size = [52, 60, 66, 56][f];
            let o = (144 - size) / 2;
            key_svg(&format!(r##"<rect x="{o}" y="{o}" width="{size}" height="{size}" rx="8" fill="#fff"/>"##), "")
        }
        Kind::Next | Kind::Prev => {
            let (a, b) = if kind == Kind::Next {
                ("M34 40l40 32-40 32z", "M70 40l40 32-40 32z")
            } else {
                ("M110 40L70 72l40 32z", "M74 40L34 72l40 32z")
            };
            let o = ["0.35", "0.6", "1", "0.55"][f];
            key_svg(&format!(r##"<path d="{a}" fill="#fff"/><path d="{b}" fill="#fff" opacity="{o}"/>"##), "")
        }
        Kind::VolUp | Kind::VolDown => {
            // Subir: las ondas se encienden una a una; bajar: se apagan
            let lit = if kind == Kind::VolUp { f.min(3) } else { 3 - f.min(3) };
            let sign = if kind == Kind::VolUp { "+" } else { "−" };
            let badge = format!(
                r##"<text x="30" y="40" font-family="sans-serif" font-size="34" font-weight="bold" fill="#fff" text-anchor="middle">{sign}</text>"##
            );
            key_svg(&format!("{SPEAKER}{}{badge}", waves(lit)), &caption_level(level))
        }
        Kind::Mute => {
            let muted = level.is_some_and(|l| l.muted);
            let extra = if muted {
                r##"<path d="M86 52l28 40M114 52L86 92" stroke="#ff5a5a" stroke-width="8" stroke-linecap="round"/>"##.to_string()
            } else {
                waves(3)
            };
            key_svg(&format!("{SPEAKER}{extra}"), &caption_level(level))
        }
        Kind::Dial => render_dial(s, level, np, None),
    }
}

/// Dibujo de la pantalla de una rueda (200×100). `shown` = volumen a mostrar
/// durante la animación (si no, el real).
pub(super) fn render_dial(s: &ActSettings, level: Option<Level>, np: Option<&NowPlaying>, shown: Option<u8>) -> String {
    render_dial_frame(s, level, np, shown, &crate::player::Frame::still())
}

/// Título y artista que se ven en la rueda (solo en el volumen general)
pub(super) fn dial_song(s: &ActSettings, level: Option<Level>, np: Option<&NowPlaying>) -> (String, String) {
    if !audio::is_general(&s.target) {
        // Programa cerrado o sin sonido: se dice en vez de dejar la barra vacía sin más
        return if level.is_none() { (crate::i18n::t("audio.not_playing").into(), String::new()) } else { (String::new(), String::new()) };
    }
    match np {
        Some(n) if s.show_title => (n.title.trim().to_string(), n.artist.trim().to_string()),
        _ => (String::new(), String::new()),
    }
}

/// Si la rueda tiene algo que se mueve (ecualizador o título que se desliza)
pub(super) fn dial_moving(s: &ActSettings, level: Option<Level>, np: Option<&NowPlaying>) -> bool {
    let playing = np.is_some_and(|n| n.playing);
    let (title, _) = dial_song(s, level, np);
    playing && (crate::player::DialStyle::from_id(&s.dial_style).has_eq() || crate::player::dial_needs_marquee(&title))
}

pub(super) fn render_dial_frame(s: &ActSettings, level: Option<Level>, np: Option<&NowPlaying>, shown: Option<u8>, f: &crate::player::Frame) -> String {
    let name = audio::display_name(&s.target);
    let (value, color, fill) = match level {
        None => ("—".to_string(), "#555", 0.0),
        Some(l) if l.muted => (crate::i18n::t("audio.muted").to_string(), "#666", 0.0),
        Some(l) => {
            let v = shown.unwrap_or(l.volume);
            let color = if v > 80 { "#ff5a5a" } else { "#4aa3ff" };
            (v.to_string(), color, v as f32 / 100.0)
        }
    };
    let (title, artist) = dial_song(s, level, np);
    crate::player::render_dial(
        &crate::player::DialView {
            name: &name,
            value: &value,
            fill,
            bar_color: color,
            title: &title,
            artist: &artist,
            style: crate::player::DialStyle::from_id(&s.dial_style),
            eq_color: &s.bar_color,
            muted: level.is_some_and(|l| l.muted),
            background: &super::outputs::background(),
        },
        f,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dibujos_validos() {
        let s = ActSettings::default();
        let np = NowPlaying { title: "Bohemian <Rhapsody> & más".into(), artist: "Queen".into(), playing: true, ..Default::default() };
        let lvl = Some(Level { volume: 64, muted: false });
        for k in [Kind::PlayPause, Kind::Stop, Kind::Next, Kind::Prev, Kind::VolUp, Kind::VolDown, Kind::Mute, Kind::Dial] {
            for f in [None, Some(0), Some(3)] {
                let svg = render_key(k, &s, lvl, Some(&np), f);
                assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
                assert!(!svg.contains("<Rhapsody>"), "texto sin escapar en {k:?}");
            }
        }
        assert!(render_dial(&s, lvl, Some(&np), None).contains(">64<"));
        assert!(render_dial(&s, lvl, None, None).contains("64 %"));
        // Silenciado: símbolo y «SILENCIO», sin barra
        let muted = render_dial(&s, Some(Level { volume: 10, muted: true }), None, None);
        assert!(muted.contains("SILENCIO") && muted.contains("<polygon") && !muted.contains(r##"fill="#2a2a2a""##));
        assert_eq!(Kind::from_uuid("com.malzar.prisma.audio.dial"), Some(Kind::Dial));
        assert_eq!(ActSettings::from_value(&json!({"target": "spotify.exe"})).step, 2);
    }
}

/// Genera los iconos de `assets/actions` a partir de los mismos dibujos:
/// `PRISMA_ICONS_OUT=assets/actions cargo test --bin prisma generar_iconos -- --ignored`
#[cfg(test)]
#[test]
#[ignore]
pub(super) fn generar_iconos() {
    let dir = std::env::var("PRISMA_ICONS_OUT").unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let s = ActSettings { show_title: false, ..Default::default() };
    let lvl = Some(Level { volume: 50, muted: false });
    let np = NowPlaying { playing: false, ..Default::default() };
    for (name, kind) in [
        ("playpause", Kind::PlayPause), ("stop", Kind::Stop), ("next", Kind::Next), ("prev", Kind::Prev),
        ("volup", Kind::VolUp), ("voldown", Kind::VolDown), ("mute", Kind::Mute),
    ] {
        // Imagen de la tecla (con fondo) e icono de la lista (sin texto)
        let key = render_key(kind, &ActSettings { target: "general".into(), ..s.clone() }, lvl, Some(&np), None);
        std::fs::write(format!("{dir}/{name}.svg"), &key).unwrap();
    }
    std::fs::write(format!("{dir}/dial.svg"), render_dial(&s, lvl, None, None)).unwrap();
    std::fs::write(
        format!("{dir}/dial-icon.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 144 144" width="144" height="144"><circle cx="72" cy="72" r="46" fill="none" stroke="#fff" stroke-width="10"/><path d="M72 30v26" stroke="#fff" stroke-width="10" stroke-linecap="round"/><path d="M30 120h84" stroke="#4aa3ff" stroke-width="10" stroke-linecap="round"/></svg>"##,
    )
    .unwrap();
}

#[cfg(test)]
#[test]
#[ignore]
pub(super) fn volcar_muestras() {
    let dir = std::env::var("PRISMA_SAMPLES_OUT").unwrap();
    let g = ActSettings::default();
    let sp = ActSettings { target: "spotify.exe".into(), ..Default::default() };
    let np = NowPlaying { title: "Bohemian Rhapsody".into(), artist: "Queen".into(), playing: true, ..Default::default() };
    let l = |v, m| Some(Level { volume: v, muted: m });
    let samples = [
        ("k1", render_key(Kind::PlayPause, &g, None, Some(&np), None)),
        ("k2", render_key(Kind::PlayPause, &ActSettings { show_title: false, ..g.clone() }, None, None, None)),
        ("k3", render_key(Kind::VolUp, &g, l(64, false), None, Some(1))),
        ("k4", render_key(Kind::VolDown, &sp, l(40, false), None, None)),
        ("k5", render_key(Kind::Mute, &g, l(64, true), None, None)),
        ("k6", render_key(Kind::Next, &g, None, None, Some(1))),
        ("d1", render_dial(&g, l(64, false), Some(&np), None)),
        ("d2", render_dial(&sp, l(86, false), None, None)),
        ("d3", render_dial(&ActSettings { target: "discord.exe".into(), ..Default::default() }, l(80, true), None, None)),
        ("d4", render_dial(&ActSettings { target: "game.exe".into(), ..Default::default() }, None, None, None)),
    ];
    for (n, s) in samples {
        std::fs::write(format!("{dir}/{n}.svg"), s).unwrap();
    }
}
