//! Luna: su forma, la fase, cuánto se ve iluminada y cuándo sale o se pone

use super::{Ctx, hm};
use crate::{
    astro::{moon_svg, phase_name},
    i18n::{t, tf},
    svg::{ACCENT, MUTED, text},
};

pub fn draw(c: &Ctx, dial: bool) -> String {
    let Some((m, _)) = c.sky else { return String::new() };
    let name = t(phase_name(m.phase));
    let pct = tf("weather.moon.lit", &[&(m.fraction * 100.0).round()]);
    let when = if m.up { format!("↑{} ↓{}", hm(m.rise), hm(m.set)) } else { tf("weather.moon.rises_at", &[&hm(m.next_rise)]) };
    let color = if m.up { ACCENT } else { MUTED };
    if dial {
        format!(
            "{}{}{}{}",
            moon_svg(27.0, 50.0, 25.0, m.fraction, m.phase),
            text(128.0, 32.0, 15.0, "#fff", true, name),
            text(128.0, 60.0, 19.0, MUTED, true, &pct),
            text(128.0, 90.0, 20.0, color, true, &when),
        )
    } else {
        format!(
            "{}{}{}{}",
            moon_svg(72.0, 40.0, 30.0, m.fraction, m.phase),
            text(72.0, 94.0, 18.0, "#fff", true, name),
            text(72.0, 116.0, 16.0, MUTED, false, &pct),
            text(72.0, 138.0, 17.0, color, true, &when),
        )
    }
}
