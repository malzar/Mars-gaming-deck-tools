//! La hora: digital o con agujas en las teclas (también solo las horas o solo
//! los minutos, para montar una columna en los laterales del MSD-ONE) y
//! digital en la rueda

use chrono::{Datelike, Local, Timelike};

use super::{
    calendar::{day_name, month_name},
    settings::WSettings,
};
use crate::{
    i18n::t,
    svg::{ACCENT, MUTED, text},
};

pub fn body(s: &WSettings, dial: bool, now: &chrono::DateTime<Local>) -> String {
    let hm = format!("{:02}:{:02}", now.hour(), now.minute());
    let day = day_name(now.weekday().num_days_from_monday() as usize);
    if dial {
        let main = if s.seconds {
            format!(
                r##"<text x="100" y="56" font-family="sans-serif" font-size="46" font-weight="bold" fill="#fff" text-anchor="middle">{hm}<tspan font-size="22" fill="{MUTED}">:{:02}</tspan></text>"##,
                now.second()
            )
        } else {
            text(100.0, 58.0, 50.0, "#fff", true, &hm)
        };
        let date = format!("{day}, {} {}", now.day(), month_name(now.month0() as usize).chars().take(3).collect::<String>());
        return format!("{main}{}", text(100.0, 88.0, 21.0, MUTED, true, &date));
    }
    if s.style == "analog" {
        return analog(now);
    }
    match s.part.as_str() {
        "hours" => format!("{}{}", text(72.0, 92.0, 76.0, "#fff", true, &format!("{:02}", now.hour())), text(72.0, 122.0, 16.0, MUTED, false, t("clock.hour"))),
        "minutes" => format!("{}{}", text(72.0, 92.0, 76.0, "#fff", true, &format!("{:02}", now.minute())), text(72.0, 122.0, 16.0, MUTED, false, t("clock.min"))),
        _ if s.show_date => format!("{}{}", text(72.0, 80.0, 44.0, "#fff", true, &hm), text(72.0, 108.0, 18.0, MUTED, false, &format!("{day} {}", now.day()))),
        _ => text(72.0, 88.0, 46.0, "#fff", true, &hm),
    }
}

/// Reloj de agujas
fn analog(now: &chrono::DateTime<Local>) -> String {
    let h = (now.hour() % 12) as f32 + now.minute() as f32 / 60.0;
    let m = now.minute() as f32 + now.second() as f32 / 60.0;
    let hand = |angle: f32, len: f32, width: f32, color: &str| {
        let a = (angle - 90.0).to_radians();
        format!(
            r##"<path d="M72 72L{:.1} {:.1}" stroke="{color}" stroke-width="{width}" stroke-linecap="round"/>"##,
            72.0 + a.cos() * len,
            72.0 + a.sin() * len
        )
    };
    let ticks: String = (0..12)
        .map(|i| {
            let a = (i as f32 * 30.0 - 90.0).to_radians();
            let (r0, r1) = if i % 3 == 0 { (52.0, 62.0) } else { (57.0, 62.0) };
            format!("M{:.1} {:.1}L{:.1} {:.1}", 72.0 + a.cos() * r0, 72.0 + a.sin() * r0, 72.0 + a.cos() * r1, 72.0 + a.sin() * r1)
        })
        .collect();
    format!(
        r##"<circle cx="72" cy="72" r="66" fill="none" stroke="#3a4558" stroke-width="4"/><path d="{ticks}" stroke="{MUTED}" stroke-width="4" stroke-linecap="round"/>{}{}{}<circle cx="72" cy="72" r="5" fill="{ACCENT}"/>"##,
        hand(h * 30.0, 36.0, 7.0, "#fff"),
        hand(m * 6.0, 54.0, 5.0, "#fff"),
        hand(now.second() as f32 * 6.0, 56.0, 2.5, ACCENT),
    )
}
