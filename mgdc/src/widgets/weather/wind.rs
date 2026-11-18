//! Viento: hacia dónde sopla, velocidad y rachas

use super::Ctx;
use crate::{
    i18n::{t, tf},
    svg::{ACCENT, MUTED, text},
    weather::{compass, wind_word},
};

pub fn draw(c: &Ctx, dial: bool) -> String {
    let w = c.w;
    if dial {
        let speed = format!(
            r##"<text x="138" y="40" font-family="sans-serif" font-size="42" font-weight="bold" fill="#fff" text-anchor="middle">{}<tspan font-size="19" fill="{MUTED}"> km/h</tspan></text>"##,
            w.wind.round()
        );
        return format!(
            "{}{speed}{}{}",
            arrow(36.0, 52.0, 20.0, w.wind_dir),
            text(136.0, 68.0, 17.0, ACCENT, true, &format!("{} · {}", t(compass(w.wind_dir)), t(wind_word(w.wind)))),
            text(138.0, 94.0, 19.0, MUTED, true, &tf("weather.wind.gusts_short", &[&w.gusts.round()])),
        );
    }
    format!(
        "{}{}{}",
        arrow(72.0, 50.0, 26.0, w.wind_dir),
        text(72.0, 112.0, 25.0, "#fff", true, &tf("weather.wind.speed", &[&w.wind.round(), &t(compass(w.wind_dir))])),
        text(72.0, 136.0, 18.0, MUTED, false, &tf("weather.wind.gusts_short", &[&w.gusts.round()])),
    )
}

/// Flecha del viento: hacia dónde va (sopla desde `deg`)
fn arrow(cx: f32, cy: f32, r: f32, deg: f32) -> String {
    let marks: String = [t("dir.n"), t("dir.e"), t("dir.s"), t("dir.w")]
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let a = (i as f32 * 90.0 - 90.0).to_radians();
            text(cx + a.cos() * (r + 10.0), cy + a.sin() * (r + 10.0) + 5.0, 14.0, MUTED, true, l)
        })
        .collect();
    format!(
        r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="none" stroke="#3a4558" stroke-width="3"/>{marks}<g transform="rotate({rot} {cx} {cy})"><path d="M{cx} {t}L{l} {b}L{cx} {m}L{rr} {b}z" fill="{ACCENT}"/></g>"##,
        rot = deg + 180.0,
        t = cy - r * 0.8,
        b = cy + r * 0.7,
        m = cy + r * 0.35,
        l = cx - r * 0.45,
        rr = cx + r * 0.45
    )
}
