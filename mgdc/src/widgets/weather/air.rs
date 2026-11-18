//! Calidad del aire: índice europeo, nivel y partículas finas

use super::Ctx;
use crate::{
    i18n::{t, tf},
    svg::{MUTED, text},
    weather::aqi_level,
};

pub fn draw(c: &Ctx, dial: bool) -> String {
    let Some(a) = c.w.air.as_ref() else { return none(dial) };
    let Some(aqi) = a.aqi else { return none(dial) };
    let (level, color) = aqi_level(aqi);
    let level = t(level);
    if dial {
        let pm = tf("weather.air.pm25", &[&a.pm25.unwrap_or(0.0).round()]);
        format!(
            r##"<circle cx="34" cy="50" r="30" fill="none" stroke="{color}" stroke-width="8"/>{}{}{}{}"##,
            text(34.0, 62.0, 32.0, color, true, &format!("{}", aqi.round())),
            text(135.0, 30.0, 18.0, MUTED, true, t("weather.air.title")),
            text(135.0, 60.0, 24.0, color, true, level),
            text(135.0, 90.0, 19.0, MUTED, true, &pm),
        )
    } else {
        format!(
            "{}{}{}",
            text(72.0, 24.0, 17.0, MUTED, true, t("weather.air.title")),
            text(72.0, 86.0, 62.0, color, true, &format!("{}", aqi.round())),
            text(72.0, 122.0, 26.0, color, true, level),
        )
    }
}

/// Sin datos del aire para ese sitio
fn none(dial: bool) -> String {
    let (cx, cy) = if dial { (100.0, 56.0) } else { (72.0, 78.0) };
    text(cx, cy, 18.0, MUTED, false, t("weather.air.none"))
}
