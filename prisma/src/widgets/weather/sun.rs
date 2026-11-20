//! Sol: el arco del día con el sol (y la luna, si también está fuera), las
//! horas de salida y puesta y cuánto dura el día. De noche, la luna.

use super::{Ctx, hm};
use crate::{
    astro::{Moon, moon_svg, phase_name},
    i18n::{t, tf},
    svg::{MUTED, esc, text},
};

pub fn draw(c: &Ctx, dial: bool) -> String {
    let w = c.w;
    let sky = c.sky;
    let today = w.days.first();
    let (rise, set) = today.map_or((String::new(), String::new()), |d| (d.sunrise.clone(), d.sunset.clone()));
    let now = w.time.get(11..16).and_then(minutes);
    let frac = match (minutes(&rise), minutes(&set), now) {
        (Some(r), Some(s), Some(n)) if n >= r && n <= s && s > r => Some((n - r) / (s - r)),
        _ => None,
    };
    let light = match (minutes(&rise), minutes(&set)) {
        (Some(r), Some(s)) if s > r => tf("weather.sun.daylight", &[&(((s - r) / 60.0) as i32), &format!("{:02}", ((s - r) % 60.0) as i32)]),
        _ => String::new(),
    };
    let moon = sky.map(|(m, _)| m);
    let moon_at = sky.and_then(|(m, now)| m.progress(*now).map(|f| (f, m)));
    let phase = moon.map(|m| t(phase_name(m.phase))).unwrap_or("");
    // De día: el sol (y la luna si también está fuera). De noche: la luna.
    let (left, right, bottom, label) = match (frac, moon) {
        (Some(_), _) => (format!("↑ {rise}"), format!("↓ {set}"), light, t("weather.sun.title")),
        (None, Some(m)) if m.up => (format!("↑ {}", hm(m.rise)), format!("↓ {}", hm(m.set)), phase.to_string(), t("weather.moon.title")),
        (None, Some(m)) => (phase.to_string(), String::new(), tf("weather.moon.rises_at", &[&hm(m.next_rise)]), t("weather.moon.title")),
        (None, None) => (format!("↑ {rise}"), format!("↓ {set}"), light, t("weather.sun.title")),
    };
    // Sol y luna fuera a la vez: las horas de los dos, con su inicial
    if let (Some(_), Some((_, m))) = (frac, moon_at) {
        let (ls, ll) = initials();
        let (mr, ms) = (hm(m.rise), hm(m.set));
        return if dial {
            format!(
                "{}{}{}",
                arc(50.0, 150.0, 40.0, 32.0, frac, moon_at),
                times(100.0, 70.0, 22.0, ls, "#ffc83d", &rise, &set),
                times(100.0, 97.0, 22.0, ll, "#f2e6b8", &mr, &ms),
            )
        } else {
            format!(
                "{}{}{}",
                arc(24.0, 120.0, 68.0, 42.0, frac, moon_at),
                times(72.0, 106.0, 19.0, ls, "#ffc83d", &rise, &set),
                times(72.0, 134.0, 19.0, ll, "#f2e6b8", &mr, &ms),
            )
        };
    }
    if dial {
        let sides = format!(
            r##"<text x="2" y="76" font-family="sans-serif" font-size="23" font-weight="bold" fill="#fff">{l}</text><text x="198" y="76" font-family="sans-serif" font-size="23" font-weight="bold" fill="{MUTED}" text-anchor="end">{r}</text>"##,
            l = esc(&left),
            r = esc(&right)
        );
        format!("{}{sides}{}", arc(46.0, 154.0, 50.0, 40.0, frac, moon_at), text(100.0, 98.0, 18.0, MUTED, true, &bottom))
    } else if right.is_empty() {
        // De noche sin luna: su fase y cuándo sale
        format!(
            "{}{}{}{}",
            text(72.0, 22.0, 17.0, MUTED, true, label),
            moon.map(|m| moon_svg(72.0, 58.0, 24.0, m.fraction, m.phase)).unwrap_or_default(),
            text(72.0, 108.0, 19.0, "#fff", true, &left),
            text(72.0, 134.0, 17.0, MUTED, false, &tf("weather.moon.rises_at_lower", &[&moon.map(|m| hm(m.next_rise)).unwrap_or_default()])),
        )
    } else {
        format!(
            "{}{}{}{}",
            text(72.0, 22.0, 17.0, MUTED, true, label),
            arc(24.0, 120.0, 74.0, 40.0, frac, moon_at),
            text(72.0, 106.0, 24.0, "#fff", true, &left),
            text(72.0, 134.0, 24.0, MUTED, true, &right),
        )
    }
}

/// Minutos desde medianoche de "HH:MM"
fn minutes(hm: &str) -> Option<f32> {
    let (h, m) = hm.split_once(':')?;
    Some(h.parse::<f32>().ok()? * 60.0 + m.get(..2)?.parse::<f32>().ok()?)
}

/// Iniciales de sol y luna en el idioma de OpenDeck (Sol/Luna, Sun/Moon…)
fn initials() -> (&'static str, &'static str) {
    (t("weather.sun.initial"), t("weather.moon.initial"))
}

/// Una fila «S ↑08:01 ↓17:52» con la inicial en su color
fn times(x: f32, y: f32, size: f32, letter: &str, color: &str, up: &str, down: &str) -> String {
    format!(
        r##"<text x="{x}" y="{y}" font-family="sans-serif" font-size="{size}" font-weight="bold" text-anchor="middle"><tspan fill="{color}">{letter}</tspan><tspan fill="#fff"> ↑{up}</tspan><tspan fill="{MUTED}"> ↓{down}</tspan></text>"##,
        up = esc(up),
        down = esc(down)
    )
}

/// Arco del sol entre la salida y la puesta, con el sol donde está ahora
fn arc(x0: f32, x1: f32, base: f32, h: f32, frac: Option<f32>, moon: Option<(f32, &Moon)>) -> String {
    let (cx, rx) = ((x0 + x1) / 2.0, (x1 - x0) / 2.0);
    let at = |f: f32| {
        let a = std::f32::consts::PI * (1.0 - f);
        (cx + a.cos() * rx, base - a.sin() * h)
    };
    let sun = match frac {
        Some(f) => {
            let (x, y) = at(f);
            format!(r##"<circle cx="{x:.1}" cy="{y:.1}" r="7" fill="#ffc83d"/>"##)
        }
        None => String::new(),
    };
    // La luna, con su forma, en su punto del recorrido
    let moon = match moon {
        Some((f, m)) => {
            let (x, y) = at(f);
            moon_svg(x, y, if frac.is_some() { 6.0 } else { 9.0 }, m.fraction, m.phase)
        }
        None => String::new(),
    };
    format!(
        r##"<path d="M{x0} {base}H{x1}" stroke="#555" stroke-width="2"/><path d="M{x0} {base}A{rx} {h} 0 0 1 {x1} {base}" fill="none" stroke="#3a4558" stroke-width="2" stroke-dasharray="3 4"/>{moon}{sun}"##
    )
}
