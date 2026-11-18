//! Polen: el que más hay (tecla) o los tres que más (rueda), con su nivel

use super::Ctx;
use crate::{
    i18n::{t, tf},
    svg::{MUTED, text},
    weather::pollen_level,
};

pub fn draw(c: &Ctx, dial: bool) -> String {
    let Some(a) = c.w.air.as_ref().filter(|a| a.has_pollen) else {
        // Open-Meteo solo tiene polen en Europa
        let (cx, cy) = if dial { (100.0, 46.0) } else { (72.0, 66.0) };
        return format!(
            "{}{}",
            text(cx, cy, 18.0, MUTED, false, t("weather.pollen.none")),
            text(cx, cy + 24.0, 16.0, MUTED, false, t("weather.pollen.europe_only"))
        );
    };
    if dial {
        return a
            .pollen
            .iter()
            .take(3)
            .enumerate()
            .map(|(i, (name, v))| {
                let (level, color) = pollen_level(name, *v);
                let (name, level) = (t(name), t(level));
                let y = 21.0 + i as f32 * 32.0;
                format!(
                    r##"<text x="4" y="{y}" font-family="sans-serif" font-size="18" font-weight="bold" fill="#fff">{name}</text><text x="196" y="{y}" font-family="sans-serif" font-size="18" font-weight="bold" fill="{color}" text-anchor="end">{level}</text><rect x="4" y="{by}" width="192" height="5" rx="2.5" fill="#1d2a3a"/><rect x="4" y="{by}" width="{bw:.1}" height="5" rx="2.5" fill="{color}"/>"##,
                    by = y + 4.0,
                    bw = (v / 200.0).clamp(0.03, 1.0) * 192.0
                )
            })
            .collect();
    }
    let (name, v) = a.pollen[0];
    let (level, color) = pollen_level(name, v);
    let (name, level) = (t(name), t(level));
    format!(
        "{}{}{}{}",
        text(72.0, 24.0, 17.0, MUTED, true, t("weather.pollen.title")),
        text(72.0, 62.0, 24.0, "#fff", true, name),
        text(72.0, 98.0, 28.0, color, true, level),
        text(72.0, 128.0, 17.0, MUTED, false, &tf("weather.pollen.grains", &[&v.round()])),
    )
}
