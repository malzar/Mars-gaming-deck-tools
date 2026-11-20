//! La pantalla de la rueda «Música»: lo que suena, si está en pausa, y al
//! girar «Siguiente» / «Anterior»
//!
//! El icono del programa se dibuja como cuadraditos de color (SVG puro): la
//! pantalla de las ruedas de OpenDeck no pinta imágenes PNG dentro del SVG.

use std::sync::Arc;

use super::{
    Frame,
    dial::{DIAL_W, DialStyle, EqArea, eq_bars},
    text::{fit, marquee_offset, text_width},
};
use crate::{i18n::t, svg::esc};

/// Píxeles RGBA de un icono cuadrado (`size`×`size`)
#[derive(Debug, Clone, PartialEq)]
pub struct Pixels {
    pub size: u32,
    pub rgba: Vec<u8>,
}

/// Dónde va el icono del programa que suena
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IconPlace {
    None,
    /// Junto al ▶ / ‖
    State,
    /// Grande y tenue, de fondo
    Background,
    /// Junto al título y el artista
    Title,
}

impl IconPlace {
    pub fn from_id(id: &str) -> IconPlace {
        match id {
            "none" => IconPlace::None,
            "background" => IconPlace::Background,
            "title" => IconPlace::Title,
            _ => IconPlace::State,
        }
    }
}

/// Qué se pinta en la rueda de música
pub struct MusicView<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub playing: bool,
    pub style: DialStyle,
    pub eq_color: &'a str,
    /// Icono del programa, dónde y si a la derecha
    pub icon: Option<Arc<Pixels>>,
    pub icon_place: IconPlace,
    pub icon_right: bool,
}

/// Letra del título y del artista (más grandes que en la rueda de volumen:
/// aquí no hay barra de volumen)
const TITLE: f32 = 20.0;
const ARTIST: f32 = 17.0;
/// Textos pequeños («Siguiente», «No suena nada»)
const SMALL: f32 = 16.0;
/// Lado del icono del programa: junto al estado, junto al título y de fondo
const ICON_STATE: f32 = 32.0;
const ICON_TITLE: f32 = 40.0;
const ICON_BG: f32 = 84.0;
/// Ancho del título y el artista con el icono al lado
const W_WITH_ICON: f32 = 136.0;

fn svg(body: &str) -> String {
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100"><rect width="200" height="100" fill="#000"/>{body}</svg>"##)
}

/// Título arriba, en la franja `x0`..`x0 + w` (se desliza si no cabe)
fn title_line(title: &str, color: &str, f: &Frame, x0: f32, w: f32) -> String {
    let tw = text_width(title, TITLE, true);
    if tw > w {
        let off = marquee_offset(tw, w, f.t);
        format!(
            r##"<defs><clipPath id="m"><rect x="{x0}" y="0" width="{w}" height="100"/></clipPath></defs><text clip-path="url(#m)" x="{x:.1}" y="27" font-family="sans-serif" font-size="{TITLE}" font-weight="bold" fill="{color}">{}</text>"##,
            esc(title),
            x = x0 - off
        )
    } else {
        format!(
            r##"<text x="{cx}" y="27" font-family="sans-serif" font-size="{TITLE}" font-weight="bold" fill="{color}" text-anchor="middle">{}</text>"##,
            esc(title),
            cx = x0 + w / 2.0
        )
    }
}

/// El icono en (x, y) con lado `size`: un cuadradito por punto de pantalla
/// (como mucho 32 de lado), juntando los seguidos del mismo color
fn image(px: &Pixels, x: f32, y: f32, size: f32, opacity: f32) -> String {
    let n = (size.round() as u32).clamp(8, 32).min(px.size.max(1));
    let cell = size / n as f32;
    let src = px.size as f32 / n as f32;
    // Color medio de cada cuadradito (en 16 tonos por canal, para poder juntar)
    let at = |c: u32, r: u32| -> Option<(u8, u8, u8, u8)> {
        let (x0, y0) = ((c as f32 * src) as u32, (r as f32 * src) as u32);
        let (x1, y1) = ((((c + 1) as f32 * src) as u32).max(x0 + 1), (((r + 1) as f32 * src) as u32).max(y0 + 1));
        let (mut sum, mut cnt) = ([0u32; 4], 0u32);
        for yy in y0..y1.min(px.size) {
            for xx in x0..x1.min(px.size) {
                let i = ((yy * px.size + xx) * 4) as usize;
                let a = px.rgba[i + 3] as u32;
                for (k, s) in sum.iter_mut().take(3).enumerate() {
                    *s += px.rgba[i + k] as u32 * a;
                }
                sum[3] += a;
                cnt += 1;
            }
        }
        let a = sum[3] / cnt.max(1);
        if a < 24 {
            return None;
        }
        // 16 tonos que llegan hasta el blanco (0x00, 0x11 … 0xff)
        let q = |v: u32| ((v.min(255) as u8) >> 4) * 17;
        let c = |k: usize| q(sum[k] / sum[3].max(1));
        Some((c(0), c(1), c(2), q(a)))
    };
    let mut out = format!(r##"<g opacity="{opacity}" shape-rendering="crispEdges">"##);
    for r in 0..n {
        let mut c = 0;
        while c < n {
            let Some(col) = at(c, r) else {
                c += 1;
                continue;
            };
            let mut end = c + 1;
            while end < n && at(end, r) == Some(col) {
                end += 1;
            }
            let (rr, gg, bb, aa) = col;
            out += &format!(
                r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#{rr:02x}{gg:02x}{bb:02x}"{}/>"##,
                x + c as f32 * cell,
                y + r as f32 * cell,
                (end - c) as f32 * cell + 0.05,
                cell + 0.05,
                if aa == 0xff { String::new() } else { format!(r#" fill-opacity="{:.2}""#, aa as f32 / 255.0) }
            );
            c = end;
        }
    }
    out + "</g>"
}

/// Sonando: ▶; en pausa: ‖ (con la esquina en x, y)
fn state_icon(playing: bool, x: f32, y: f32) -> String {
    if playing {
        format!(r##"<polygon points="{x},{y} {x},{} {},{}" fill="#fff"/>"##, y + 28.0, x + 24.0, y + 14.0)
    } else {
        format!(r##"<rect x="{x}" y="{y}" width="8" height="28" fill="#9a9a9a"/><rect x="{}" y="{y}" width="8" height="28" fill="#9a9a9a"/>"##, x + 15.0)
    }
}

pub fn render_music_dial(v: &MusicView, f: &Frame) -> String {
    let title = v.title.trim();
    if title.is_empty() {
        // No suena nada: zzz, dormida
        return svg(&format!(
            r##"<g font-family="sans-serif" font-weight="bold" fill="#666"><text x="76" y="64" font-size="36">Z</text><text x="106" y="44" font-size="26">z</text><text x="126" y="28" font-size="18">z</text></g><text x="100" y="92" font-family="sans-serif" font-size="{SMALL}" fill="#888" text-anchor="middle">{}</text>"##,
            esc(t("audio.music_idle"))
        ));
    }
    let (fg, sub) = if v.playing { ("#fff", "#bbb") } else { ("#9a9a9a", "#777") };
    let fade = if v.playing { 1.0 } else { 0.5 };
    let icon = v.icon.as_deref().filter(|_| v.icon_place != IconPlace::None);
    let at = |place| icon.filter(|_| v.icon_place == place);
    let mut body = String::new();
    // De fondo, el ecualizador o el icono: no los dos
    if let Some(px) = at(IconPlace::Background).filter(|_| v.style != DialStyle::EqBackground) {
        body += &image(px, 100.0 - ICON_BG / 2.0, 8.0, ICON_BG, 0.22);
    }
    let eq = v.playing && v.style.has_eq();
    if eq && v.style == DialStyle::EqBackground {
        body += &eq_bars(&f.eq, EqArea { bars: 12, x: 0.0, w: 200.0, bottom: 100.0, max_h: 70.0 }, v.eq_color, 0.28);
    }
    // Título y artista (con el icono al lado, si va ahí)
    let (x0, w) = match at(IconPlace::Title) {
        Some(px) => {
            body += &image(px, if v.icon_right { 196.0 - ICON_TITLE } else { 4.0 }, 8.0, ICON_TITLE, fade);
            (if v.icon_right { 10.0 } else { 54.0 }, W_WITH_ICON)
        }
        None => (12.0, DIAL_W),
    };
    body += &title_line(title, fg, f, x0, w);
    if !v.artist.trim().is_empty() {
        body += &format!(
            r##"<text x="{cx}" y="50" font-family="sans-serif" font-size="{ARTIST}" fill="{sub}" text-anchor="middle">{}</text>"##,
            esc(&fit(v.artist.trim(), ARTIST, false, w)),
            cx = x0 + w / 2.0
        );
    }
    // Abajo: el estado (y el icono junto a él, si va ahí)
    let by_state = at(IconPlace::State);
    if eq && v.style == DialStyle::Eq {
        // Estado, ecualizador al lado e icono en un extremo
        let (icon_x, state_x, eq_x) = match by_state {
            Some(_) if v.icon_right => (196.0 - ICON_STATE, 10.0, 42.0),
            Some(_) => (4.0, 44.0, 76.0),
            None => (0.0, 12.0, 44.0),
        };
        if let Some(px) = by_state {
            body += &image(px, icon_x, 62.0, ICON_STATE, fade);
        }
        body += &state_icon(v.playing, state_x, 64.0);
        let eq_w = if by_state.is_some() { 112.0 } else { 144.0 };
        body += &eq_bars(&f.eq, EqArea { bars: 10, x: eq_x, w: eq_w, bottom: 92.0, max_h: 28.0 }, v.eq_color, 1.0);
    } else {
        if let Some(px) = by_state {
            body += &image(px, if v.icon_right { 122.0 } else { 46.0 }, 62.0, ICON_STATE, fade);
        }
        body += &state_icon(v.playing, if v.playing { 89.0 } else { 88.0 }, 64.0);
    }
    svg(&body)
}

/// Al girar: la pista anterior o siguiente, un momento
pub fn render_music_skip(title: &str, next: bool) -> String {
    let arrows = if next {
        r##"<polygon points="70,40 70,72 94,56"/><polygon points="94,40 94,72 118,56"/><rect x="120" y="40" width="7" height="32"/>"##
    } else {
        r##"<rect x="73" y="40" width="7" height="32"/><polygon points="106,40 106,72 82,56"/><polygon points="130,40 130,72 106,56"/>"##
    };
    let title = fit(title.trim(), TITLE, true, DIAL_W);
    svg(&format!(
        r##"<text x="100" y="27" font-family="sans-serif" font-size="{TITLE}" font-weight="bold" fill="#9a9a9a" text-anchor="middle">{}</text><g fill="#4aa3ff">{arrows}</g><text x="100" y="94" font-family="sans-serif" font-size="{SMALL}" fill="#4aa3ff" text-anchor="middle">{}</text>"##,
        esc(&title),
        esc(t(if next { "audio.music_next" } else { "audio.music_prev" }))
    ))
}

/// Si la rueda de música tiene algo que se mueve
pub fn music_moving(v: &MusicView) -> bool {
    let w = if v.icon.is_some() && v.icon_place == IconPlace::Title { W_WITH_ICON } else { DIAL_W };
    v.playing && !v.title.trim().is_empty() && (v.style.has_eq() || text_width(v.title.trim(), TITLE, true) > w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rueda_de_musica() {
        let f = Frame::still();
        let v = |title, playing, style| MusicView {
            title,
            artist: "Queen",
            playing,
            style,
            eq_color: "#2ecc71",
            icon: None,
            icon_place: IconPlace::State,
            icon_right: false,
        };
        for s in [
            render_music_dial(&v("", false, DialStyle::Bar), &f),
            render_music_dial(&v("Bohemian Rhapsody", true, DialStyle::Bar), &f),
            render_music_dial(&v("Bohemian Rhapsody", false, DialStyle::Eq), &f),
            render_music_dial(&v("Una canción con un título larguísimo que no cabe", true, DialStyle::EqBackground), &f),
            render_music_skip("A & B", true),
            render_music_skip("", false),
        ] {
            assert!(s.starts_with("<svg") && s.ends_with("</svg>"), "{s}");
        }
        assert!(render_music_dial(&v("", true, DialStyle::Bar), &f).contains(">Z<"));
        // Icono del programa en cada sitio y lado (rojo, con un hueco transparente)
        let mut rgba = [255u8, 0, 0, 255].repeat(48 * 48);
        rgba[3] = 0;
        let red = Arc::new(Pixels { size: 48, rgba });
        for place in [IconPlace::State, IconPlace::Background, IconPlace::Title] {
            for right in [false, true] {
                for style in [DialStyle::Bar, DialStyle::Eq] {
                    let m = MusicView { icon: Some(red.clone()), icon_place: place, icon_right: right, ..v("Hola", true, style) };
                    assert!(render_music_dial(&m, &f).contains("#ff0000"));
                }
            }
        }
        let both = MusicView { icon: Some(red.clone()), icon_place: IconPlace::Background, ..v("Hola", true, DialStyle::EqBackground) };
        assert!(!render_music_dial(&both, &f).contains("#ff0000"));
        let none = MusicView { icon: Some(red), icon_place: IconPlace::None, ..v("Hola", true, DialStyle::Bar) };
        assert!(!render_music_dial(&none, &f).contains("#ff0000"));
        assert!(music_moving(&v("x", true, DialStyle::Eq)));
        assert!(!music_moving(&v("x", false, DialStyle::Eq)));
        assert!(!music_moving(&v("", true, DialStyle::Eq)));
    }
}

#[cfg(test)]
#[test]
#[ignore]
fn volcar_musica() {
    let dir = std::env::var("OUT").unwrap();
    let mut rgba = vec![];
    for y in 0..48u32 {
        for x in 0..48u32 {
            let d = ((x as i32 - 24).pow(2) + (y as i32 - 24).pow(2)) as f32;
            rgba.extend(if d < 22.0 * 22.0 { [30, (x * 5) as u8, 90 + y as u8 * 3, 255] } else { [0, 0, 0, 0] });
        }
    }
    let px = Arc::new(Pixels { size: 48, rgba });
    let f = Frame::still();
    for (i, (place, right, style)) in [
        (IconPlace::State, false, DialStyle::Bar),
        (IconPlace::State, true, DialStyle::Eq),
        (IconPlace::Title, false, DialStyle::Bar),
        (IconPlace::Background, false, DialStyle::Bar),
        (IconPlace::Title, true, DialStyle::Eq),
        (IconPlace::State, false, DialStyle::Eq),
    ]
    .into_iter()
    .enumerate()
    {
        let v = MusicView { title: "Bohemian Rhapsody", artist: "Queen", playing: true, style, eq_color: "#2ecc71", icon: Some(px.clone()), icon_place: place, icon_right: right };
        std::fs::write(format!("{dir}/m{i}.svg"), render_music_dial(&v, &f)).unwrap();
    }
}
