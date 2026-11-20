//! La tecla Reproducir/Pausa con lo que suena

use super::{
    Bar, Frame, KEY_BARS, Look,
    text::{fit, marquee_offset, two_lines, text_width},
};
use crate::{audio::NowPlaying, svg::esc};

pub(super) const WIDTH: f32 = 132.0;
pub(super) const TITLE: f32 = 26.0;
pub(super) const MARQUEE: f32 = 30.0;

/// Si el título no cabe y se tiene que deslizar
pub fn needs_marquee(np: &NowPlaying, _small: bool) -> bool {
    np.playing && two_lines(np.title.trim(), TITLE, WIDTH).is_none()
}

/// Si la tecla tiene algo que se mueve ahora mismo
pub fn moving(np: Option<&NowPlaying>, look: &Look) -> bool {
    np.is_some_and(|n| n.playing && (look.bar.animated() || needs_marquee(n, look.small)))
}

// ------------------------------------------------------------------ barras --

fn bar_svg(look: &Look, f: &Frame) -> String {
    let c = esc(look.color);
    match look.bar {
        Bar::None => String::new(),
        Bar::Color => format!(r##"<rect y="134" width="144" height="10" fill="{c}"/>"##),
        Bar::Waves => (0..15)
            .map(|i| {
                let h = 2.0 + 9.0 * ((i as f32) * 0.55 + f.t * 4.0).sin().abs();
                format!(
                    r##"<rect x="{x:.1}" y="{y:.1}" width="6" height="{h:.1}" rx="3" fill="{c}"/>"##,
                    x = 6.0 + i as f32 * 9.5,
                    y = 139.0 - h / 2.0
                )
            })
            .collect(),
        Bar::Eq => f
            .eq
            .iter()
            .take(KEY_BARS)
            .enumerate()
            .map(|(i, v)| {
                let h = 3.0 + v.clamp(0.0, 1.0) * 31.0;
                format!(
                    r##"<rect x="{x}" y="{y:.1}" width="15" height="{h:.1}" rx="2" fill="{c}"/>"##,
                    x = 6 + i * 19,
                    y = 142.0 - h
                )
            })
            .collect(),
        // Se dibuja de fondo, antes que el texto (ver `background_svg`)
        Bar::EqBackground => String::new(),
    }
}

/// Alto que ocupa la barra abajo (lo que queda libre para el texto)
fn bar_height(look: &Look) -> f32 {
    match look.bar {
        Bar::None => 0.0,
        Bar::Eq => 36.0,
        Bar::EqBackground => 0.0,
        _ => 16.0,
    }
}

/// Ecualizador de fondo: barras altas y suaves que ocupan toda la tecla
fn background_svg(look: &Look, f: &Frame) -> String {
    if look.bar != Bar::EqBackground {
        return String::new();
    }
    let c = esc(look.color);
    f.eq.iter()
        .take(KEY_BARS)
        .enumerate()
        .map(|(i, v)| {
            let h = 8.0 + v.clamp(0.0, 1.0) * 128.0;
            format!(
                r##"<rect x="{x:.1}" y="{y:.1}" width="18" height="{h:.1}" rx="3" fill="{c}" opacity="0.3"/>"##,
                x = 2.0 + i as f32 * 20.3,
                y = 144.0 - h
            )
        })
        .collect()
}

// ------------------------------------------------------------------ tecla --

/// Dibujo de la tecla con lo que suena. `pressed` = atenuada al pulsarla.
pub fn render(np: Option<&NowPlaying>, look: &Look, f: &Frame, pressed: bool) -> String {
    let body = match np {
        // Sonando: título grande, artista y barra
        Some(n) if n.playing => {
            let title = n.title.trim();
            let artist = n.artist.trim();
            let room = 144.0 - bar_height(look);
            // Con el ecualizador alto abajo, todo un poco más junto
            let tight = room < 120.0;
            let artist_y = room - if tight { 12.0 } else { 18.0 };
            let artist_svg = if artist.is_empty() {
                String::new()
            } else {
                format!(
                    r##"<text x="72" y="{artist_y:.0}" font-family="sans-serif" font-size="20" fill="#bbb" text-anchor="middle">{}</text>"##,
                    esc(&fit(artist, 20.0, false, WIDTH))
                )
            };
            // Sin artista, el título baja un poco para quedar centrado
            let shift = if artist.is_empty() { 12.0 } else { 0.0 };
            let title_svg = match two_lines(title, TITLE, WIDTH) {
                Some((a, b)) if b.is_empty() => format!(
                    r##"<text x="72" y="{y:.0}" font-family="sans-serif" font-size="{TITLE}" font-weight="bold" fill="#fff" text-anchor="middle">{}</text>"##,
                    esc(&a),
                    y = artist_y - 40.0 + shift
                ),
                Some((a, b)) => format!(
                    r##"<text font-family="sans-serif" font-size="{TITLE}" font-weight="bold" fill="#fff" text-anchor="middle"><tspan x="72" y="{y1:.0}">{}</tspan><tspan x="72" y="{y2:.0}">{}</tspan></text>"##,
                    esc(&a),
                    esc(&b),
                    y1 = artist_y - if tight { 59.0 } else { 66.0 } + shift,
                    y2 = artist_y - if tight { 29.0 } else { 32.0 } + shift
                ),
                // No cabe: una línea que se desliza
                None => {
                    let w = text_width(title, MARQUEE, true);
                    format!(
                        r##"<defs><clipPath id="m"><rect x="6" y="0" width="{WIDTH}" height="144"/></clipPath></defs><text clip-path="url(#m)" x="{x:.1}" y="{y:.0}" font-family="sans-serif" font-size="{MARQUEE}" font-weight="bold" fill="#fff">{}</text>"##,
                        esc(title),
                        x = 8.0 - marquee_offset(w, WIDTH, f.t),
                        y = artist_y - 38.0 + shift
                    )
                }
            };
            format!("{}{title_svg}{artist_svg}{}", background_svg(look, f), bar_svg(look, f))
        }
        // En pausa: ▶ y el título en pequeño
        Some(n) if !n.title.trim().is_empty() => {
            let artist = n.artist.trim();
            let lines = if artist.is_empty() {
                format!(r##"<tspan x="72" y="118">{}</tspan>"##, esc(&fit(&n.title, 17.0, false, WIDTH)))
            } else {
                format!(
                    r##"<tspan x="72" y="108">{}</tspan><tspan x="72" y="128">{}</tspan>"##,
                    esc(&fit(&n.title, 17.0, false, WIDTH)),
                    esc(&fit(artist, 17.0, false, WIDTH))
                )
            };
            format!(
                r##"<path d="M54 26l40 28-40 28z" fill="#fff"/><text font-family="sans-serif" font-size="17" fill="#bbb" text-anchor="middle">{lines}</text>"##
            )
        }
        // No suena nada
        _ => r##"<path d="M50 40l48 32-48 32z" fill="#fff"/>"##.to_string(),
    };
    let dim = if pressed { r#" opacity="0.6""# } else { "" };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 144 144" width="144" height="144"><rect width="144" height="144" fill="#000"/><g{dim}>{body}</g></svg>"##
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    fn np(title: &str, playing: bool) -> NowPlaying {
        NowPlaying { title: title.into(), artist: "Queen".into(), playing, ..Default::default() }
    }

    #[test]
    fn titulos() {
        assert!(two_lines("Bohemian Rhapsody", TITLE, WIDTH).is_some());
        assert!(two_lines("Never Gonna Give You Up", TITLE, WIDTH).is_none());
        assert!(needs_marquee(&np("Never Gonna Give You Up", true), false));
        assert_eq!(marquee_offset(300.0, WIDTH, 0.0), 0.0);
        assert!(marquee_offset(300.0, WIDTH, 3.0) > 0.0);
    }

    #[test]
    fn deck_pequeno() {
        let look = Look { bar: Bar::Eq, color: "#2ecc71", small: false };
        assert!(moving(Some(&np("Hola", true)), &look));
        assert!(!moving(Some(&np("Hola", false)), &look));
    }

/// Muestras para revisar el dibujo a ojo:
/// PRISMA_SAMPLES_OUT=carpeta cargo test muestras_tecla -- --ignored
#[cfg(test)]
#[test]
#[ignore]
fn muestras_tecla() {
    let dir = std::env::var("PRISMA_SAMPLES_OUT").unwrap();
    let np = |t: &str, a: &str, p: bool| NowPlaying { title: t.into(), artist: a.into(), playing: p, ..Default::default() };
    let look = |bar, small| Look { bar, color: "#2ecc71", small };
    let eq = [0.3, 0.7, 1.0, 0.5, 0.8, 0.2, 0.6, 0.45, 0.9, 0.35, 0.75, 0.55];
    let fr = |t| Frame { t, eq };
    let s = [
        ("1-nada", render(Some(&np("Bohemian Rhapsody", "Queen", true)), &look(Bar::None, false), &fr(0.0), false)),
        ("2-color", render(Some(&np("Bohemian Rhapsody", "Queen", true)), &look(Bar::Color, false), &fr(0.0), false)),
        ("3-ondas", render(Some(&np("Bohemian Rhapsody", "Queen", true)), &look(Bar::Waves, false), &fr(0.3), false)),
        ("4-eq", render(Some(&np("Bohemian Rhapsody", "Queen", true)), &look(Bar::Eq, false), &fr(0.0), false)),
        ("4b-eq-fondo", render(Some(&np("Bohemian Rhapsody", "Queen", true)), &look(Bar::EqBackground, false), &fr(0.0), false)),
        ("4c-eq-fondo-largo", render(Some(&np("Never Gonna Give You Up", "Rick Astley", true)), &look(Bar::EqBackground, false), &fr(3.0), false)),
        ("5-corto", render(Some(&np("Hello", "Adele", true)), &look(Bar::Color, false), &fr(0.0), false)),
        ("6-largo-a", render(Some(&np("Never Gonna Give You Up", "Rick Astley", true)), &look(Bar::Eq, false), &fr(0.5), false)),
        ("7-largo-b", render(Some(&np("Never Gonna Give You Up", "Rick Astley", true)), &look(Bar::Eq, false), &fr(4.0), false)),
        ("8-pausa", render(Some(&np("Bohemian Rhapsody", "Queen", false)), &look(Bar::Eq, false), &fr(0.0), false)),
        ("9-nada", render(None, &look(Bar::Eq, false), &fr(0.0), false)),
        ("10-one-largo", render(Some(&np("Never Gonna Give You Up", "Rick Astley", true)), &look(Bar::Eq, true), &fr(0.0), false)),
        ("11-sin-artista", render(Some(&np("Lofi beats to relax", "", true)), &look(Bar::Waves, false), &fr(0.0), false)),
    ];
    for (n, svg) in s {
        std::fs::write(format!("{dir}/{n}.svg"), svg).unwrap();
    }
}
}
