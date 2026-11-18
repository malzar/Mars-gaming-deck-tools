//! Índice UV: ahora, nivel, consejo y máximo de hoy

use super::Ctx;
use crate::{
    i18n::{t, tf},
    svg::{MUTED, text},
    weather::uv_level,
};

pub fn draw(c: &Ctx, dial: bool) -> String {
    let w = c.w;
    let max = w.days.first().map_or(w.uv, |d| d.uv_max);
    let (level, color, tip) = uv_level(w.uv);
    let (level, tip) = (t(level), t(tip));
    if dial {
        format!(
            r##"<circle cx="34" cy="50" r="30" fill="none" stroke="{color}" stroke-width="8"/>{}{}{}{}"##,
            text(34.0, 65.0, 44.0, color, true, &format!("{}", w.uv.round())),
            text(135.0, 34.0, 24.0, color, true, level),
            text(135.0, 64.0, 16.0, "#fff", true, tip),
            text(135.0, 92.0, 18.0, MUTED, true, &tf("weather.uv.max_today", &[&max.round()])),
        )
    } else {
        format!(
            "{}{}{}{}",
            text(72.0, 24.0, 17.0, MUTED, true, t("weather.uv.title")),
            text(72.0, 84.0, 62.0, color, true, &format!("{}", w.uv.round())),
            text(72.0, 112.0, 24.0, color, true, level),
            text(72.0, 137.0, 16.0, "#fff", false, tip),
        )
    }
}
