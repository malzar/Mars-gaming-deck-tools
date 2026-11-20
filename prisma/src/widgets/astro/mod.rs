//! Astronomía: una vista por astro (el Sol, la Luna, los planetas y una
//! estrella a elegir). Cada una dice si está fuera ahora y a qué altura,
//! cuándo sale y se pone y en qué constelación está (o a qué distancia, los
//! planetas). En tecla, el Sol y la Luna tienen dos páginas, que van en la
//! rueda de vistas como una vista más; en la rueda caben en una.
//!
//! - `sun`: las dos formas del Sol (eventos del día o fases de luz)
//! - Aquí: la Luna, los planetas y las estrellas (cada una elegida es una
//!   vista más, «star:Nombre»), y las piezas comunes

mod sun;

use chrono::{DateTime, Local, Utc};

use super::settings::WSettings;
use crate::{
    astro::{moon_altitude, moon_phase, moon_radec, moon_svg, phase_name},
    i18n::{t, tf},
    sky::{self, Planet, RAD, RiseSet},
    svg::{MUTED, clip, text},
};

/// Las vistas, en el orden de la ventana de ajustes
pub const VIEWS: &[&str] = &["sun", "moon", "mercury", "venus", "mars", "jupiter", "saturn", "uranus", "neptune", "star"];

/// Lo que se enseña si no hay ninguna elegida
const DEFAULT: &[&str] = &["sun", "moon"];

/// Lo que va pasando con la rueda o al pulsar: las vistas elegidas, y en
/// tecla el Sol y la Luna con su segunda página justo detrás
pub fn slots(s: &WSettings, dial: bool) -> Vec<String> {
    let chosen: Vec<&str> = s.views.iter().map(String::as_str).filter(|v| VIEWS.contains(v) || v.starts_with("star:")).collect();
    let chosen = if chosen.is_empty() { DEFAULT.to_vec() } else { chosen };
    let mut out = vec![];
    for v in chosen {
        out.push(v.to_string());
        if !dial && (v == "sun" || v == "moon") {
            out.push(format!("{v}:2"));
        }
    }
    out
}

const GREEN: &str = "#2ecc71";

/// Planeta, su color y la clave de su nombre
fn planet(id: &str) -> Option<(Planet, &'static str, &'static str)> {
    Some(match id {
        "mercury" => (Planet::Mercury, "#b9b0a6", "astro.mercury"),
        "venus" => (Planet::Venus, "#f3e2b0", "astro.venus"),
        "mars" => (Planet::Mars, "#e8603a", "astro.mars"),
        "jupiter" => (Planet::Jupiter, "#e6c08f", "astro.jupiter"),
        "saturn" => (Planet::Saturn, "#e8d49a", "astro.saturn"),
        "uranus" => (Planet::Uranus, "#9fe3e8", "astro.uranus"),
        "neptune" => (Planet::Neptune, "#6f8ff5", "astro.neptune"),
        _ => return None,
    })
}

/// Hora local "HH:MM" (o "--:--")
fn hm(t: Option<DateTime<Utc>>) -> String {
    t.map(|t| t.with_timezone(&Local).format("%H:%M").to_string()).unwrap_or_else(|| "--:--".into())
}

/// Un número con su coma o punto según el idioma
fn num(x: f64, decimals: usize) -> String {
    let s = format!("{x:.decimals$}");
    if crate::i18n::code() == "es" { s.replace('.', ",") } else { s }
}

/// «Visible · 23°» o «No visible · −12°»
fn seen(alt: f64) -> (String, &'static str) {
    let deg = alt.round() as i32;
    let deg = if deg < 0 { format!("−{}", -deg) } else { deg.to_string() };
    if alt > 0.0 { (tf("astro.up", &[&deg]), GREEN) } else { (tf("astro.down", &[&deg]), MUTED) }
}

/// «↑ 07:12» y «↓ 18:40», o una sola línea si no sale o no se pone
fn rise_set_lines(rs: &RiseSet) -> Result<(String, String), String> {
    if rs.always_up {
        Err(t("astro.always_up").into())
    } else if rs.never_up {
        Err(t("astro.never_up").into())
    } else {
        Ok((format!("↑ {}", hm(rs.rise)), format!("↓ {}", hm(rs.set))))
    }
}

/// Lo que se sabe de un astro para dibujarlo
struct Body {
    name: String,
    color: &'static str,
    alt: f64,
    rs: RiseSet,
    /// Constelación o distancia
    extra: String,
}

/// Página de un astro: nombre, si se ve, salida, puesta y lo de abajo
fn draw_body(b: &Body, dial: bool, icon: &str) -> String {
    let (vis, vis_color) = seen(b.alt);
    let times = rise_set_lines(&b.rs);
    if dial {
        let times = match times {
            Ok((r, s)) => format!("{r}  {s}"),
            Err(one) => one,
        };
        return format!(
            "{icon}{}{}{}{}",
            text(42.0, 88.0, 15.0, b.color, true, &clip(&b.name, 10)),
            text(128.0, 26.0, 15.0, vis_color, true, &vis),
            text(128.0, 56.0, 18.0, "#fff", true, &times),
            text(128.0, 86.0, 15.0, MUTED, false, &clip(&b.extra, 16)),
        );
    }
    let mid = match times {
        Ok((r, s)) => format!("{}{}", text(72.0, 84.0, 20.0, "#fff", true, &r), text(72.0, 108.0, 20.0, "#fff", true, &s)),
        Err(one) => text(72.0, 96.0, 17.0, "#fff", true, &one),
    };
    format!(
        "{}{}{mid}{}",
        text(72.0, 30.0, 22.0, b.color, true, &clip(&b.name, 11)),
        text(72.0, 56.0, 16.0, vis_color, true, &vis),
        text(72.0, 134.0, 15.0, MUTED, false, &clip(&b.extra, 18)),
    )
}

/// Dibujo de una de las `slots` (cuerpo sin el `<svg>`)
pub fn body(slot: &str, dial: bool, s: &WSettings, lat: f64, lon: f64, now: DateTime<Utc>) -> String {
    let (id, page2) = match slot.strip_suffix(":2") {
        Some(id) => (id, true),
        None => (slot, false),
    };
    match id {
        "sun" => sun::draw(dial, page2, &s.sun_mode, lat, lon, now),
        "moon" => moon(dial, page2, lat, lon, now),
        // «star» solo: la de antes, cuando solo había una
        "star" => star(dial, &s.star, lat, lon, now),
        _ if id.starts_with("star:") => star(dial, &id[5..], lat, lon, now),
        _ => match planet(id) {
            Some((p, color, key)) => {
                let (ra, dec, dist) = sky::planet_position(p, now);
                let at = |t| {
                    let (ra, dec, _) = sky::planet_position(p, t);
                    sky::altitude(ra, dec, t, lat, lon)
                };
                let b = Body {
                    name: t(key).into(),
                    color,
                    alt: sky::altitude(ra, dec, now, lat, lon),
                    rs: sky::rise_set(now, -0.5667, at),
                    extra: tf("astro.au", &[&num(dist, 2)]),
                };
                let icon = format!(r##"<circle cx="42" cy="44" r="18" fill="{color}"/>{}"##, if p == Planet::Saturn { ring() } else { String::new() });
                draw_body(&b, dial, &icon)
            }
            None => String::new(),
        },
    }
}

/// El anillo de Saturno (rueda)
fn ring() -> String {
    r##"<ellipse cx="42" cy="44" rx="26" ry="7" fill="none" stroke="#c9b27a" stroke-width="3" transform="rotate(-18 42 44)"/>"##.into()
}

/// La Luna: en tecla, la fase en una página y lo demás en otra
fn moon(dial: bool, page2: bool, lat: f64, lon: f64, now: DateTime<Utc>) -> String {
    let (fraction, phase) = moon_phase(now);
    let (ra, dec, _) = moon_radec(now);
    let b = Body {
        name: t("astro.moon").into(),
        color: "#f2e6b8",
        alt: moon_altitude(now, lat, lon),
        rs: sky::rise_set(now, 0.133, |t| moon_altitude(t, lat, lon)),
        extra: sky::constellation(ra, dec).into(),
    };
    let name = t(phase_name(phase));
    let lit = tf("weather.moon.lit", &[&(fraction * 100.0).round()]);
    if dial {
        // Sin sitio para «Visible»: el color dice si está fuera
        let (_, vis_color) = seen(b.alt);
        let alt = seen(b.alt).0.rsplit(" · ").next().unwrap_or_default().to_string();
        let times = rise_set_lines(&b.rs).map_or_else(|one| one, |(r, s)| format!("{r}  {s}"));
        return format!(
            "{}{}{}{}{}",
            moon_svg(30.0, 50.0, 26.0, fraction, phase),
            text(130.0, 20.0, 15.0, "#fff", true, &clip(name, 17)),
            text(130.0, 43.0, 15.0, vis_color, true, &format!("{} % · {alt}", (fraction * 100.0).round())),
            text(130.0, 68.0, 17.0, "#fff", true, &times),
            text(130.0, 92.0, 14.0, MUTED, false, &clip(&b.extra, 18)),
        );
    }
    if page2 {
        return draw_body(&b, false, "");
    }
    format!("{}{}{}", moon_svg(72.0, 52.0, 36.0, fraction, phase), text(72.0, 112.0, 16.0, "#fff", false, &clip(name, 17)), text(72.0, 134.0, 16.0, MUTED, false, &lit))
}

/// La estrella elegida (o un aviso para elegirla)
fn star(dial: bool, name: &str, lat: f64, lon: f64, now: DateTime<Utc>) -> String {
    let Some(st) = sky::find_star(name) else {
        let (cx, cy) = if dial { (100.0, 56.0) } else { (72.0, 76.0) };
        return text(cx, cy, 16.0, MUTED, false, t("astro.pick_star"));
    };
    let (ra, dec) = (st.ra * RAD, st.dec * RAD);
    let b = Body {
        name: st.name.into(),
        color: "#fff",
        alt: sky::altitude(ra, dec, now, lat, lon),
        rs: sky::rise_set(now, -0.5667, |t| sky::altitude(ra, dec, t, lat, lon)),
        extra: st.con.into(),
    };
    draw_body(&b, dial, &star_icon(42.0, 44.0, 18.0))
}

/// Estrella de cuatro puntas
fn star_icon(cx: f32, cy: f32, r: f32) -> String {
    let k = r * 0.28;
    format!(
        r##"<path d="M{cx} {t}L{a} {b}L{rr} {cy}L{a} {c}L{cx} {bo}L{d} {c}L{l} {cy}L{d} {b}Z" fill="#fff"/>"##,
        t = cy - r,
        bo = cy + r,
        l = cx - r,
        rr = cx + r,
        a = cx + k,
        d = cx - k,
        b = cy - k,
        c = cy + k
    )
}

#[cfg(test)]
pub use tests::samples;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::wrap;
    use chrono::TimeZone;

    #[test]
    fn paginas() {
        let mut s = WSettings::default();
        assert_eq!(slots(&s, false), vec!["sun", "sun:2", "moon", "moon:2"]);
        assert_eq!(slots(&s, true), vec!["sun", "moon"]);
        s.views = vec!["mars".into(), "weather".into(), "moon".into(), "star:Vega".into(), "star:Sirius".into()];
        assert_eq!(slots(&s, false), vec!["mars", "moon", "moon:2", "star:Vega", "star:Sirius"]);
    }

    /// Vistas de muestra en SVG (las saca `muestras_widgets`)
    pub fn samples(out: &mut Vec<(String, String)>) {
        let now = Utc.with_ymd_and_hms(2026, 11, 20, 16, 30, 0).unwrap();
        let mut s = WSettings { star: "Vega".into(), ..Default::default() };
        for mode in ["events", "light"] {
            s.sun_mode = mode.into();
            for slot in ["sun", "sun:2"] {
                out.push((format!("astro-{}-{mode}", slot.replace(':', "-")), wrap(false, &body(slot, false, &s, 40.42, -3.70, now))));
            }
            out.push((format!("astro-rueda-sun-{mode}"), wrap(true, &body("sun", true, &s, 40.42, -3.70, now))));
        }
        for v in VIEWS.iter().skip(1).copied().chain(["moon:2"]) {
            out.push((format!("astro-{}", v.replace(':', "-")), wrap(false, &body(v, false, &s, 40.42, -3.70, now))));
            out.push((format!("astro-rueda-{}", v.replace(':', "-")), wrap(true, &body(v, true, &s, 40.42, -3.70, now))));
        }
    }
}
