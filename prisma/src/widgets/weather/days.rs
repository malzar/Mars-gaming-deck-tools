//! Previsión: los próximos días, un día suelto (hoy, mañana, pasado) y, en la
//! rueda, por horas

use super::{Ctx, temp};
use crate::{
    i18n::t,
    svg::{MUTED, text},
    weather::{self, Weather},
    widgets::calendar::weekday_of,
};

/// Una columna de la previsión: día, icono, máxima y mínima
fn column(w: &Weather, i: usize, label: &str, x: f32, icon: f32, sizes: (f32, f32, f32), ys: (f32, f32, f32, f32)) -> String {
    let Some(d) = w.days.get(i) else { return String::new() };
    let label = if label.is_empty() { weekday_of(&d.date) } else { label.to_string() };
    format!(
        "{}{}{}{}",
        text(x, ys.0, sizes.0, MUTED, true, &label),
        weather::icon(d.code, true, x - icon / 2.0, ys.1, icon),
        text(x, ys.2, sizes.1, "#fff", true, &temp(d.max)),
        text(x, ys.3, sizes.2, MUTED, false, &temp(d.min)),
    )
}

/// Próximos días: 3 en la tecla (desde mañana), 4 en la rueda (desde hoy)
pub fn forecast(c: &Ctx, dial: bool) -> String {
    if dial {
        return (0..4)
            .map(|i| column(c.w, i, if i == 0 { t("weather.today") } else { "" }, 25.0 + i as f32 * 50.0, 40.0, (15.0, 22.0, 18.0), (15.0, 17.0, 77.0, 98.0)))
            .collect();
    }
    (1..4).map(|i| column(c.w, i, "", 24.0 + (i - 1) as f32 * 48.0, 42.0, (17.0, 23.0, 19.0), (26.0, 32.0, 100.0, 126.0))).collect()
}

/// Cada 2 horas, 4 columnas (solo en la rueda)
pub fn hours(c: &Ctx, _dial: bool) -> String {
    c.w.hours
        .iter()
        .step_by(2)
        .take(4)
        .enumerate()
        .map(|(i, h)| {
            let x = 25.0 + i as f32 * 50.0;
            let hour = h.time.get(11..13).and_then(|x| x.parse::<u32>().ok()).unwrap_or(12);
            format!(
                "{}{}{}",
                text(x, 18.0, 18.0, MUTED, true, &format!("{hour}h")),
                weather::icon(h.code, (7..20).contains(&hour), x - 21.0, 20.0, 42.0),
                text(x, 92.0, 24.0, "#fff", true, &temp(h.temp)),
            )
        })
        .collect()
}

pub fn today(c: &Ctx, _dial: bool) -> String {
    one_day(c.w, 0, t("weather.today"))
}

pub fn tomorrow(c: &Ctx, _dial: bool) -> String {
    one_day(c.w, 1, t("weather.tomorrow"))
}

pub fn after(c: &Ctx, _dial: bool) -> String {
    one_day(c.w, 2, t("weather.after"))
}

/// Un día grande en la tecla
fn one_day(w: &Weather, i: usize, label: &str) -> String {
    let Some(d) = w.days.get(i) else { return String::new() };
    let temps = format!(
        r##"<text x="72" y="132" font-family="sans-serif" font-size="30" text-anchor="middle" font-weight="bold"><tspan fill="#fff">{}</tspan> <tspan fill="{MUTED}">{}</tspan></text>"##,
        temp(d.max),
        temp(d.min)
    );
    format!("{}{}{temps}", text(72.0, 24.0, 20.0, MUTED, true, label), weather::icon(d.code, true, 38.0, 30.0, 68.0))
}
