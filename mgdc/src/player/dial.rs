//! La pantalla de las ruedas de volumen: el programa, el volumen y, si es el
//! general, lo que suena (con ecualizador si se quiere)

use super::{
    EQ_BARS, Frame,
    text::{fit, marquee_offset, text_width},
};
use crate::svg::esc;

/// Cómo se ve la pantalla de la rueda
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DialStyle {
    /// Título y artista arriba, barra de volumen y número
    Bar,
    /// El ecualizador en lugar de la barra; el volumen, en una línea fina debajo
    Eq,
    /// Ecualizador suave de fondo, detrás de todo
    EqBackground,
}

impl DialStyle {
    pub fn from_id(id: &str) -> DialStyle {
        match id {
            "eq" => DialStyle::Eq,
            "eqbg" => DialStyle::EqBackground,
            _ => DialStyle::Bar,
        }
    }

    pub fn has_eq(self) -> bool {
        self != DialStyle::Bar
    }
}

const DIAL_W: f32 = 176.0;
const DIAL_TITLE: f32 = 17.0;

/// Dónde van las barras del ecualizador
struct EqArea {
    /// Cuántas barras
    bars: usize,
    /// Izquierda y ancho de la zona
    x: f32,
    w: f32,
    /// Línea de abajo y alto máximo de una barra
    bottom: f32,
    max_h: f32,
}

fn eq_bars(eq: &[f32; EQ_BARS], area: EqArea, color: &str, opacity: f32) -> String {
    let EqArea { bars: n, x: x0, w, bottom, max_h } = area;
    let bw = w / n as f32;
    eq.iter()
        .take(n)
        .enumerate()
        .map(|(i, v)| {
            let h = 2.0 + v.clamp(0.0, 1.0) * (max_h - 2.0);
            format!(
                r##"<rect x="{x:.1}" y="{y:.1}" width="{bw2:.1}" height="{h:.1}" rx="1.5" fill="{c}" opacity="{opacity}"/>"##,
                x = x0 + i as f32 * bw + 1.5,
                y = bottom - h,
                bw2 = bw - 3.0,
                c = esc(color)
            )
        })
        .collect()
}

/// Si el título de la rueda no cabe y se tiene que deslizar
pub fn dial_needs_marquee(title: &str) -> bool {
    text_width(title, DIAL_TITLE, true) > DIAL_W
}

/// Pantalla de la rueda.
/// - `name`: "General" o el programa; `value`: el número ("64", "Silencio", "—")
/// - `fill`: de 0 a 1 lo lleno de la barra; `bar_color`: azul, o rojo si va alto
/// - `title`/`artist`: lo que suena (vacío = sin título: nombre y número grandes)
pub struct DialView<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub fill: f32,
    pub bar_color: &'a str,
    pub title: &'a str,
    pub artist: &'a str,
    pub style: DialStyle,
    pub eq_color: &'a str,
    /// Silenciado: sin barra ni ecualizador, con el símbolo de silencio
    pub muted: bool,
}

/// Altavoz tachado (silencio) de `h` de alto con la esquina en x, y
fn mute_icon(x: f32, y: f32, h: f32, color: &str) -> String {
    let k = h / 24.0;
    let p = |a: f32, b: f32| format!("{:.1},{:.1}", x + a * k, y + b * k);
    format!(
        r##"<polygon points="{} {} {} {} {} {}" fill="{c}"/><path d="M{} L{} M{} L{}" stroke="{c}" stroke-width="{w:.1}" stroke-linecap="round"/>"##,
        p(0.0, 8.0), p(6.0, 8.0), p(13.0, 2.0), p(13.0, 22.0), p(6.0, 16.0), p(0.0, 16.0),
        p(17.0, 8.0), p(25.0, 16.0), p(25.0, 8.0), p(17.0, 16.0),
        c = esc(color),
        w = 2.6 * k
    )
}

pub fn render_dial(v: &DialView, f: &Frame) -> String {
    let style = v.style;
    let mut body = String::new();
    if style == DialStyle::EqBackground && !v.muted {
        body += &eq_bars(&f.eq, EqArea { bars: 12, x: 0.0, w: 200.0, bottom: 100.0, max_h: 70.0 }, v.eq_color, 0.28);
    }
    let title = v.title.trim();
    if title.is_empty() {
        // Sin título: nombre y número grandes; debajo barra gruesa o ecualizador
        let unit = if v.value.chars().all(|c| c.is_ascii_digit()) { " %" } else { "" };
        body += &format!(
            r##"<text x="12" y="34" font-family="sans-serif" font-size="22" font-weight="bold" fill="#fff">{}</text>"##,
            esc(&fit(v.name, 22.0, true, if v.muted { 176.0 } else { 90.0 }))
        );
        if !v.muted {
            body += &format!(
                r##"<text x="188" y="36" font-family="sans-serif" font-size="30" font-weight="bold" fill="#fff" text-anchor="end">{}{unit}</text>"##,
                esc(v.value)
            );
        }
        if v.muted {
            body += &mute_icon(12.0, 52.0, 30.0, "#ff5a5a");
            body += &format!(r##"<text x="56" y="76" font-family="sans-serif" font-size="20" font-weight="bold" fill="#ff5a5a">{}</text>"##, crate::i18n::t("audio.muted"));
        } else if style == DialStyle::Eq {
            body += &eq_bars(&f.eq, EqArea { bars: 12, x: 12.0, w: DIAL_W, bottom: 82.0, max_h: 30.0 }, v.eq_color, 1.0);
            body += &volume_line(v, 88.0);
        } else {
            body += &format!(
                r##"<rect x="12" y="58" width="{DIAL_W}" height="16" rx="8" fill="#2a2a2a"/><rect x="12" y="58" width="{w:.1}" height="16" rx="8" fill="{c}"/>"##,
                w = DIAL_W * v.fill.clamp(0.0, 1.0),
                c = esc(v.bar_color)
            );
        }
    } else {
        // Con título: título (se desliza si no cabe) y artista arriba
        body += &if dial_needs_marquee(title) {
            let w = text_width(title, DIAL_TITLE, true);
            let off = marquee_offset(w, DIAL_W, f.t);
            format!(
                r##"<defs><clipPath id="d"><rect x="12" y="0" width="{DIAL_W}" height="100"/></clipPath></defs><text clip-path="url(#d)" x="{x:.1}" y="27" font-family="sans-serif" font-size="{DIAL_TITLE}" font-weight="bold" fill="#fff">{}</text>"##,
                esc(title),
                x = 12.0 - off
            )
        } else {
            format!(
                r##"<text x="100" y="27" font-family="sans-serif" font-size="{DIAL_TITLE}" font-weight="bold" fill="#fff" text-anchor="middle">{}</text>"##,
                esc(title)
            )
        };
        if !v.artist.trim().is_empty() {
            body += &format!(
                r##"<text x="100" y="48" font-family="sans-serif" font-size="16" fill="#bbb" text-anchor="middle">{}</text>"##,
                esc(&fit(v.artist, 16.0, false, DIAL_W))
            );
        }
        if v.muted {
            body += &mute_icon(12.0, 62.0, 24.0, "#ff5a5a");
            body += &format!(r##"<text x="48" y="82" font-family="sans-serif" font-size="18" font-weight="bold" fill="#ff5a5a">{}</text>"##, crate::i18n::t("audio.muted"));
        } else {
            body += &format!(
                r##"<text x="188" y="82" font-family="sans-serif" font-size="24" font-weight="bold" fill="#fff" text-anchor="end">{}</text>"##,
                esc(v.value)
            );
        }
        if v.muted {
            // Sin barra: el símbolo de silencio ocupa su sitio
        } else if style == DialStyle::Eq {
            body += &eq_bars(&f.eq, EqArea { bars: 8, x: 12.0, w: 128.0, bottom: 80.0, max_h: 18.0 }, v.eq_color, 1.0);
            body += &volume_line(v, 88.0);
        } else {
            body += &format!(
                r##"<rect x="12" y="66" width="128" height="12" rx="6" fill="#2a2a2a"/><rect x="12" y="66" width="{w:.1}" height="12" rx="6" fill="{c}"/>"##,
                w = 128.0 * v.fill.clamp(0.0, 1.0),
                c = esc(v.bar_color)
            );
        }
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100"><rect width="200" height="100" fill="#000"/>{body}</svg>"##
    )
}

/// Línea fina con el volumen (cuando el ecualizador ocupa el sitio de la barra)
fn volume_line(v: &DialView, y: f32) -> String {
    format!(
        r##"<rect x="12" y="{y}" width="{DIAL_W}" height="5" rx="2.5" fill="#2a2a2a"/><rect x="12" y="{y}" width="{w:.1}" height="5" rx="2.5" fill="{c}"/>"##,
        w = DIAL_W * v.fill.clamp(0.0, 1.0),
        c = esc(v.bar_color)
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        audio::NowPlaying,
        player::{Bar, Look, render},
    };

/// PRISMA_SAMPLES_OUT=carpeta cargo test muestras_rueda -- --ignored
#[cfg(test)]
#[test]
#[ignore]
fn muestras_rueda() {
    let dir = std::env::var("PRISMA_SAMPLES_OUT").unwrap();
    let eq = [0.3, 0.7, 1.0, 0.5, 0.8, 0.2, 0.6, 0.45, 0.9, 0.35, 0.75, 0.55];
    let f = Frame { t: 0.0, eq, gif: None };
    let v = |style, title: &'static str, artist: &'static str| DialView {
        name: "General",
        value: "64",
        fill: 0.64,
        bar_color: "#4aa3ff",
        title,
        artist,
        style,
        eq_color: "#2ecc71",
        muted: false,
    };
    let m = |style, title: &'static str| DialView { muted: true, value: "Silencio", fill: 0.0, ..v(style, title, if title.is_empty() { "" } else { "Queen" }) };
    let s = [
        ("m1-silencio", render_dial(&m(DialStyle::Bar, "Bohemian Rhapsody"), &f)),
        ("m2-silencio-fondo", render_dial(&m(DialStyle::EqBackground, "Bohemian Rhapsody"), &f)),
        ("m3-silencio-sin-titulo", render_dial(&m(DialStyle::Eq, ""), &f)),
        ("r1-barra", render_dial(&v(DialStyle::Bar, "Bohemian Rhapsody", "Queen"), &f)),
        ("r2-eq", render_dial(&v(DialStyle::Eq, "Bohemian Rhapsody", "Queen"), &f)),
        ("r3-fondo", render_dial(&v(DialStyle::EqBackground, "Bohemian Rhapsody", "Queen"), &f)),
        ("r4-largo", render_dial(&v(DialStyle::Eq, "Never Gonna Give You Up (Official Video)", "Rick Astley"), &Frame { t: 3.0, eq, gif: None })),
        ("r5-sin-titulo-barra", render_dial(&v(DialStyle::Bar, "", ""), &f)),
        ("r6-sin-titulo-eq", render_dial(&v(DialStyle::Eq, "", ""), &f)),
        ("r7-sin-titulo-fondo", render_dial(&v(DialStyle::EqBackground, "", ""), &f)),
    ];
    for (n, svg) in s {
        std::fs::write(format!("{dir}/{n}.svg"), svg).unwrap();
    }
    let np = NowPlaying { title: "Never Gonna Give You Up".into(), artist: "Rick Astley".into(), playing: true, ..Default::default() };
    let look = Look { bar: Bar::Eq, color: "#2ecc71", small: true };
    std::fs::write(format!("{dir}/k-one-eq.svg"), render(Some(&np), &look, &Frame { t: 3.0, eq, gif: None }, false)).unwrap();
}
}
