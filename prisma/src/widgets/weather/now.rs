//! «Ahora»: icono, temperatura y (en la rueda) máxima, mínima y lluvia de hoy

use super::{Ctx, drop, temp};
use crate::{
    svg::{ACCENT, MUTED, clip, text},
    weather,
};

pub fn draw(c: &Ctx, dial: bool) -> String {
    let w = c.w;
    if dial {
        let today = w.days.first();
        let range = today.map(|d| format!("↑{} ↓{}", temp(d.max), temp(d.min))).unwrap_or_default();
        let rain = today.map_or(0, |d| d.rain);
        return format!(
            "{}{}{}{}{}",
            weather::icon(w.code, w.is_day, 0.0, 8.0, 84.0),
            text(142.0, 46.0, 50.0, "#fff", true, &temp(w.temp)),
            text(142.0, 72.0, 21.0, MUTED, true, &range),
            drop(118.0, 82.0),
            text(150.0, 97.0, 21.0, ACCENT, true, &format!("{rain} %")),
        );
    }
    format!(
        "{}{}{}",
        weather::icon(w.code, w.is_day, 34.0, 0.0, 76.0),
        text(72.0, 112.0, 46.0, "#fff", true, &temp(w.temp)),
        text(72.0, 137.0, 18.0, MUTED, false, &clip(c.city, 13)),
    )
}
