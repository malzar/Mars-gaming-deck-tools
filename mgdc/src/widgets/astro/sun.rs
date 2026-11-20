//! El Sol: una página como la de los demás astros (si se ve, salida y
//! puesta) y otra, a elegir, con los eventos del día (mediodía solar, altura
//! máxima y horas de luz) o con las fases de luz (la de ahora con sus horas,
//! las dos siguientes y una barra con el día entero). En la rueda, todo junto.

use chrono::{DateTime, Duration, Local, NaiveTime, TimeZone, Utc};

use super::{Body, draw_body, hm, seen};
use crate::{
    astro::sun_radec,
    i18n::{t, tf},
    sky::{
        self,
        sun::{Light, SunDay, light_spans, sun_day},
    },
    svg::{MUTED, text},
};

const YELLOW: &str = "#ffc83d";

/// Las horas del Sol del día de hoy (el de aquí)
fn today(lat: f64, lon: f64, now: DateTime<Utc>) -> SunDay {
    let noon = now.with_timezone(&Local).date_naive().and_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap_or_default());
    let noon = Local.from_local_datetime(&noon).earliest().map_or(now, |t| t.with_timezone(&Utc));
    sun_day(noon, lat, lon)
}

fn color(l: Light) -> &'static str {
    match l {
        Light::Night => "#0d1330",
        Light::Astronomical => "#1d2a5c",
        Light::Nautical => "#2f4a94",
        Light::Blue => "#3f7be0",
        Light::Golden => "#f2a93b",
        Light::Day => "#8fcdf5",
    }
}

fn name(l: Light) -> &'static str {
    t(match l {
        Light::Night => "astro.light.night",
        Light::Astronomical => "astro.light.astronomical",
        Light::Nautical => "astro.light.nautical",
        Light::Blue => "astro.light.blue",
        Light::Golden => "astro.light.golden",
        Light::Day => "astro.light.day",
    })
}

fn short(l: Light) -> &'static str {
    t(match l {
        Light::Night => "astro.light_short.night",
        Light::Astronomical => "astro.light_short.astronomical",
        Light::Nautical => "astro.light_short.nautical",
        Light::Blue => "astro.light_short.blue",
        Light::Golden => "astro.light_short.golden",
        Light::Day => "astro.light_short.day",
    })
}

/// La fase de luz de ahora (o la siguiente) y las que vienen después, con sus
/// horas (hoy y mañana seguidos)
fn phases(lat: f64, lon: f64, now: DateTime<Utc>) -> Vec<(Light, DateTime<Utc>, DateTime<Utc>)> {
    let a = today(lat, lon, now);
    let b = today(lat, lon, now + Duration::days(1));
    let mut all: Vec<_> = light_spans(&a);
    for x in light_spans(&b) {
        match all.last_mut() {
            // La noche de hoy sigue en la de mañana
            Some(last) if last.0 == x.0 => last.2 = x.2,
            _ => all.push(x),
        }
    }
    all.into_iter().skip_while(|x| x.2 <= now).collect()
}

/// Barra con el día entero (de medianoche a medianoche) y una raya en ahora
fn bar(x: f32, y: f32, w: f32, h: f32, lat: f64, lon: f64, now: DateTime<Utc>) -> String {
    let day = today(lat, lon, now);
    let local = now.with_timezone(&Local);
    let start = Local.from_local_datetime(&local.date_naive().and_time(NaiveTime::MIN)).earliest().map_or(now, |t| t.with_timezone(&Utc));
    let span = 86_400.0;
    let px = |t: DateTime<Utc>| x + w * ((t - start).num_seconds() as f32 / span).clamp(0.0, 1.0);
    let mut out = format!(r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="3" fill="{}"/>"##, color(Light::Night));
    for (l, a, b) in light_spans(&day) {
        let (x1, x2) = (px(a), px(b));
        if x2 > x1 {
            out += &format!(r##"<rect x="{x1:.1}" y="{y}" width="{:.1}" height="{h}" fill="{}"/>"##, x2 - x1, color(l));
        }
    }
    let n = px(now);
    out + &format!(r##"<rect x="{:.1}" y="{}" width="3" height="{}" rx="1" fill="#fff"/>"##, n - 1.5, y - 3.0, h + 6.0)
}

/// Fases de luz: la de ahora con sus horas y las dos siguientes
fn light(dial: bool, lat: f64, lon: f64, now: DateTime<Utc>) -> String {
    let p = phases(lat, lon, now);
    let Some(&(cur, a, b)) = p.first() else { return String::new() };
    let range = format!("{} – {}", hm(Some(a)), hm(Some(b)));
    let tint = if cur == Light::Night { MUTED } else { color(cur) };
    let next = |i: usize| p.get(i).map(|&(l, a, _)| (l, hm(Some(a))));
    if dial {
        let mut out = bar(6.0, 6.0, 188.0, 10.0, lat, lon, now);
        out += &text(100.0, 40.0, 17.0, tint, true, name(cur));
        out += &text(100.0, 64.0, 19.0, "#fff", true, &range);
        for (i, x) in [(1, 52.0), (2, 148.0)] {
            if let Some((l, at)) = next(i) {
                out += &text(x, 91.0, 14.0, MUTED, false, &format!("{} {at}", short(l)));
            }
        }
        return out;
    }
    let mut out = bar(8.0, 8.0, 128.0, 12.0, lat, lon, now);
    out += &text(72.0, 48.0, 17.0, tint, true, name(cur));
    out += &text(72.0, 74.0, 19.0, "#fff", true, &range);
    for (i, y) in [(1, 108.0), (2, 132.0)] {
        if let Some((l, at)) = next(i) {
            out += &text(72.0, y, 16.0, MUTED, false, &format!("{} {at}", short(l)));
        }
    }
    out
}

/// Horas de luz: «9 h 45 min de luz»
fn day_length(d: &SunDay) -> String {
    d.day_length().map_or_else(String::new, |l| tf("weather.sun.daylight", &[&l.num_hours(), &format!("{:02}", l.num_minutes() % 60)]))
}

/// Altura máxima del Sol hoy
fn max_alt(d: &SunDay, lat: f64, lon: f64) -> f64 {
    let (ra, dec) = sun_radec(d.noon);
    sky::altitude(ra, dec, d.noon, lat, lon)
}

pub fn draw(dial: bool, page2: bool, mode: &str, lat: f64, lon: f64, now: DateTime<Utc>) -> String {
    let d = today(lat, lon, now);
    let (ra, dec) = sun_radec(now);
    let b = Body {
        name: t("astro.sun").into(),
        color: YELLOW,
        alt: sky::altitude(ra, dec, now, lat, lon),
        rs: sky::RiseSet { rise: d.rise_set.morning, set: d.rise_set.evening, always_up: false, never_up: false },
        extra: day_length(&d),
    };
    // Sol de medianoche o noche polar: sin salida ni puesta hoy
    let b = Body { rs: sky::RiseSet { always_up: d.rise_set.morning.is_none() && b.alt > 0.0, never_up: d.rise_set.morning.is_none() && b.alt <= 0.0, ..b.rs }, ..b };
    let transit = tf("astro.transit", &[&hm(Some(d.noon))]);
    let top = tf("astro.max_alt", &[&(max_alt(&d, lat, lon).round() as i32)]);
    if dial {
        if mode == "light" {
            return light(true, lat, lon, now);
        }
        let (vis, vis_color) = seen(b.alt);
        return format!(
            r##"<circle cx="34" cy="40" r="20" fill="{YELLOW}"/>{}{}{}{}{}"##,
            text(34.0, 88.0, 15.0, YELLOW, true, &b.name),
            text(132.0, 20.0, 15.0, vis_color, true, &vis),
            text(132.0, 45.0, 18.0, "#fff", true, &format!("↑ {}  ↓ {}", hm(b.rs.rise), hm(b.rs.set))),
            text(132.0, 69.0, 15.0, "#fff", false, &format!("{transit} · {}", top.rsplit(' ').next().unwrap_or_default())),
            text(132.0, 92.0, 14.0, MUTED, false, &b.extra),
        );
    }
    if !page2 {
        return draw_body(&b, false, "");
    }
    if mode == "light" {
        return light(false, lat, lon, now);
    }
    format!(
        "{}{}{}{}",
        text(72.0, 30.0, 16.0, MUTED, false, t("astro.transit_title")),
        text(72.0, 62.0, 28.0, YELLOW, true, &hm(Some(d.noon))),
        text(72.0, 98.0, 17.0, "#fff", true, &top),
        text(72.0, 128.0, 15.0, MUTED, false, &b.extra),
    )
}
