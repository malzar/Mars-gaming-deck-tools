//! Brillo de día y de noche: amanecer/puesta de sol y transiciones suaves.

use chrono::{DateTime, Local, NaiveDate, NaiveTime, Timelike, Utc};

use crate::settings::{DeviceSettings, ScheduleMode};

/// Amanecer y puesta de sol (UTC) para una fecha, con la ecuación del amanecer
/// estándar (precisión de un par de minutos). `lon` positiva al este.
/// Devuelve None en latitudes polares cuando no amanece o no anochece.
pub fn sun_times(lat: f64, lon: f64, date: NaiveDate) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let rad = std::f64::consts::PI / 180.0;
    let noon = date.and_hms_opt(12, 0, 0)?.and_utc();
    let jdate = noon.timestamp() as f64 / 86400.0 + 2440587.5;

    let n = (jdate - 2451545.0 + 0.0008).ceil();
    let j_star = n - lon / 360.0;
    let m = (357.5291 + 0.98560028 * j_star).rem_euclid(360.0);
    let c =
        1.9148 * (m * rad).sin() + 0.0200 * (2.0 * m * rad).sin() + 0.0003 * (3.0 * m * rad).sin();
    let lambda = (m + c + 180.0 + 102.9372).rem_euclid(360.0);
    let j_transit =
        2451545.0 + j_star + 0.0053 * (m * rad).sin() - 0.0069 * (2.0 * lambda * rad).sin();
    let decl = ((lambda * rad).sin() * (23.4397 * rad).sin()).asin();
    let cos_w =
        ((-0.833 * rad).sin() - (lat * rad).sin() * decl.sin()) / ((lat * rad).cos() * decl.cos());

    if !(-1.0..=1.0).contains(&cos_w) {
        return None;
    }

    let w = cos_w.acos() / rad;
    let to_dt =
        |j: f64| DateTime::<Utc>::from_timestamp(((j - 2440587.5) * 86400.0).round() as i64, 0);

    Some((to_dt(j_transit - w / 360.0)?, to_dt(j_transit + w / 360.0)?))
}

fn minutes_of<T: Timelike>(t: &T) -> i32 {
    (t.hour() * 60 + t.minute()) as i32
}

fn parse_hhmm(s: &str) -> Option<i32> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M")
        .ok()
        .map(|t| minutes_of(&t))
}

/// (inicio del día, inicio de la noche) en minutos desde medianoche, hora local
pub fn day_window(s: &DeviceSettings, now: &DateTime<Local>) -> (i32, i32) {
    let sc = &s.schedule;
    let fixed = (
        parse_hhmm(&sc.day_start).unwrap_or(7 * 60 + 30),
        parse_hhmm(&sc.night_start).unwrap_or(21 * 60),
    );

    if sc.mode == ScheduleMode::Sun {
        if let Some((rise, set)) = sun_times(sc.lat, sc.lon, now.date_naive()) {
            let rise = minutes_of(&rise.with_timezone(&Local)) + sc.offset_minutes;
            let set = minutes_of(&set.with_timezone(&Local)) + sc.offset_minutes;
            if rise < set {
                return (rise, set);
            }
        }
    }

    fixed
}

/// Brillo que toca ahora mismo según los ajustes del deck
pub fn target_brightness(s: &DeviceSettings, now: &DateTime<Local>) -> u8 {
    let sc = &s.schedule;
    if !sc.enabled {
        return s.brightness.min(100);
    }

    let (day, night) = day_window(s, now);
    let m = minutes_of(now);
    let t = sc.transition_minutes as i32;
    let d = sc.day_brightness.min(100) as f64;
    let n = sc.night_brightness.min(100) as f64;
    let lerp = |a: f64, b: f64, x: f64| (a + (b - a) * x.clamp(0.0, 1.0)).round() as u8;

    if t > 0 && m >= day && m < day + t {
        return lerp(n, d, (m - day) as f64 / t as f64);
    }
    if m >= day && m < night {
        return d.round() as u8;
    }
    if t > 0 && m >= night && m < night + t {
        return lerp(d, n, (m - night) as f64 / t as f64);
    }
    n.round() as u8
}

/// "HH:MM" local de amanecer y puesta para mostrarlos en el panel
pub fn sun_times_local(lat: f64, lon: f64) -> Option<(String, String)> {
    let today = Local::now().date_naive();
    let (rise, set) = sun_times(lat, lon, today)?;
    Some((
        rise.with_timezone(&Local).format("%H:%M").to_string(),
        set.with_timezone(&Local).format("%H:%M").to_string(),
    ))
}
