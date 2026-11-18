//! La luna: dónde está, cuándo sale y se pone y en qué fase está. Se calcula
//! aquí mismo, sin pedir nada a internet.
//!
//! Fórmulas de SunCalc (https://github.com/mourner/suncalc), reescritas en
//! Rust. Copyright (c) 2026, Volodymyr Agafonkin. Licencia BSD de 2 cláusulas;
//! el texto completo está en THIRD_PARTY.md.

use chrono::{DateTime, Duration, Utc};
use std::f64::consts::PI;

const RAD: f64 = PI / 180.0;
const E: f64 = RAD * 23.4397;

fn days(t: DateTime<Utc>) -> f64 {
    t.timestamp_millis() as f64 / 86_400_000.0 - 0.5 + 2_440_588.0 - 2_451_545.0
}

fn right_ascension(l: f64, b: f64) -> f64 {
    (l.sin() * E.cos() - b.tan() * E.sin()).atan2(l.cos())
}

fn declination(l: f64, b: f64) -> f64 {
    (b.sin() * E.cos() + b.cos() * E.sin() * l.sin()).asin()
}

fn sun_coords(d: f64) -> (f64, f64) {
    let m = RAD * (357.5291 + 0.985_600_28 * d);
    let c = RAD * (1.9148 * m.sin() + 0.02 * (2.0 * m).sin() + 0.0003 * (3.0 * m).sin());
    let l = m + c + RAD * 102.9372 + PI;
    (right_ascension(l, 0.0), declination(l, 0.0))
}

/// (ascensión recta, declinación, distancia en km)
fn moon_coords(d: f64) -> (f64, f64, f64) {
    let l = RAD * (218.316 + 13.176_396 * d);
    let m = RAD * (134.963 + 13.064_993 * d);
    let f = RAD * (93.272 + 13.229_350 * d);
    let lon = l + RAD * 6.289 * m.sin();
    let lat = RAD * 5.128 * f.sin();
    (right_ascension(lon, lat), declination(lon, lat), 385_001.0 - 20_905.0 * m.cos())
}

/// Altura de la luna sobre el horizonte (grados)
pub fn moon_altitude(t: DateTime<Utc>, lat: f64, lon: f64) -> f64 {
    let d = days(t);
    let (ra, dec, _) = moon_coords(d);
    let h = RAD * (280.16 + 360.985_623_5 * d) - RAD * -lon - ra;
    let phi = RAD * lat;
    (phi.sin() * dec.sin() + phi.cos() * dec.cos() * h.cos()).asin() / RAD
}

/// Fase: (parte iluminada 0-1, fase 0-1: 0 nueva, 0.25 cuarto creciente,
/// 0.5 llena, 0.75 cuarto menguante)
pub fn moon_phase(t: DateTime<Utc>) -> (f64, f64) {
    let d = days(t);
    let (sra, sdec) = sun_coords(d);
    let (mra, mdec, dist) = moon_coords(d);
    let sdist = 149_598_000.0;
    let phi = (sdec.sin() * mdec.sin() + sdec.cos() * mdec.cos() * (sra - mra).cos()).acos();
    let inc = (sdist * phi.sin()).atan2(dist - sdist * phi.cos());
    let angle = (sdec.cos() * (sra - mra).sin()).atan2(sdec.sin() * mdec.cos() - sdec.cos() * mdec.sin() * (sra - mra).cos());
    let fraction = (1.0 + inc.cos()) / 2.0;
    let phase = 0.5 + 0.5 * inc * if angle < 0.0 { -1.0 } else { 1.0 } / PI;
    (fraction, phase)
}

/// Clave del nombre de la fase
pub fn phase_name(phase: f64) -> &'static str {
    match phase {
        p if !(0.03..0.97).contains(&p) => "moon.new",
        p if p < 0.22 => "moon.waxing_crescent",
        p if p < 0.28 => "moon.first_quarter",
        p if p < 0.47 => "moon.waxing_gibbous",
        p if p < 0.53 => "moon.full",
        p if p < 0.72 => "moon.waning_gibbous",
        p if p < 0.78 => "moon.last_quarter",
        _ => "moon.waning_crescent",
    }
}

/// Dónde está la luna ahora
#[derive(Debug, Clone)]
pub struct Moon {
    pub up: bool,
    /// Si está fuera: cuándo salió y cuándo se pondrá
    pub rise: Option<DateTime<Utc>>,
    pub set: Option<DateTime<Utc>>,
    /// Si no está fuera: cuándo sale
    pub next_rise: Option<DateTime<Utc>>,
    pub fraction: f64,
    pub phase: f64,
}

impl Moon {
    /// Parte del recorrido de hoy que lleva hecha (0-1), si está fuera
    pub fn progress(&self, now: DateTime<Utc>) -> Option<f32> {
        let (r, s) = (self.rise?, self.set?);
        let total = (s - r).num_seconds() as f32;
        (total > 0.0).then(|| ((now - r).num_seconds() as f32 / total).clamp(0.0, 1.0))
    }
}

/// Busca cuándo cruza el horizonte, de 10 en 10 minutos (hacia atrás o adelante)
fn crossing(now: DateTime<Utc>, lat: f64, lon: f64, forward: bool, rising: bool) -> Option<DateTime<Utc>> {
    let step = Duration::minutes(if forward { 10 } else { -10 });
    // La luna «sale» cuando su borde pasa el horizonte (con la refracción, ~0,13°)
    let alt = |t| moon_altitude(t, lat, lon) + 0.133;
    let mut t = now;
    let mut a = alt(t);
    for _ in 0..(30 * 6) {
        let t2 = t + step;
        let a2 = alt(t2);
        // Hacia delante: de abajo a arriba es salida. Hacia atrás, al revés.
        let (before, after) = if forward { (a, a2) } else { (a2, a) };
        if (rising && before < 0.0 && after >= 0.0) || (!rising && before >= 0.0 && after < 0.0) {
            // Interpolación lineal entre los dos puntos
            let k = (0.0 - before) / (after - before);
            let (t0, t1) = if forward { (t, t2) } else { (t2, t) };
            return Some(t0 + Duration::milliseconds(((t1 - t0).num_milliseconds() as f64 * k) as i64));
        }
        t = t2;
        a = a2;
    }
    None
}

pub fn moon(now: DateTime<Utc>, lat: f64, lon: f64) -> Moon {
    let up = moon_altitude(now, lat, lon) + 0.133 >= 0.0;
    let (fraction, phase) = moon_phase(now);
    if up {
        Moon { up, rise: crossing(now, lat, lon, false, true), set: crossing(now, lat, lon, true, false), next_rise: None, fraction, phase }
    } else {
        Moon { up, rise: None, set: None, next_rise: crossing(now, lat, lon, true, true), fraction, phase }
    }
}

/// Dibujo de la luna con su fase (lo iluminado a la derecha cuando crece,
/// como se ve desde el hemisferio norte)
pub fn moon_svg(cx: f32, cy: f32, r: f32, fraction: f64, phase: f64) -> String {
    let dark = format!(r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="#2a3242" stroke="#55607a" stroke-width="1"/>"##);
    if fraction < 0.02 {
        return dark;
    }
    if fraction > 0.98 {
        return format!(r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="#f2e6b8"/>"##);
    }
    let waxing = phase < 0.5;
    let rx = r * (1.0 - 2.0 * fraction as f32).abs();
    let (top, bottom) = (cy - r, cy + r);
    // Borde iluminado (media circunferencia) y vuelta por la línea de sombra
    let limb = if waxing { 1 } else { 0 };
    let term = match (waxing, fraction < 0.5) {
        (true, true) | (false, false) => 0,
        _ => 1,
    };
    format!(
        r##"{dark}<path d="M{cx} {top}A{r} {r} 0 0 {limb} {cx} {bottom}A{rx:.2} {r} 0 0 {term} {cx} {top}z" fill="#f2e6b8"/>"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn fases_conocidas() {
        // Luna llena: 5 nov 2025 13:19 UTC; luna nueva: 20 nov 2025 06:47 UTC
        let (f, p) = moon_phase(Utc.with_ymd_and_hms(2025, 11, 5, 13, 0, 0).unwrap());
        assert!(f > 0.98 && (p - 0.5).abs() < 0.03, "{f} {p}");
        assert_eq!(phase_name(p), "moon.full");
        let (f, p) = moon_phase(Utc.with_ymd_and_hms(2025, 11, 20, 7, 0, 0).unwrap());
        assert!(f < 0.02, "{f} {p}");
        assert_eq!(phase_name(p), "moon.new");
        // Cuarto creciente: 28 nov 2025 06:59 UTC
        let (_, p) = moon_phase(Utc.with_ymd_and_hms(2025, 11, 28, 7, 0, 0).unwrap());
        assert_eq!(phase_name(p), "moon.first_quarter");
    }

    #[test]
    fn sale_y_se_pone() {
        // Madrid: siempre hay una salida en las próximas 30 h si no está fuera
        let now = Utc.with_ymd_and_hms(2026, 11, 16, 20, 0, 0).unwrap();
        let m = moon(now, 40.4168, -3.7038);
        if m.up {
            assert!(m.rise.unwrap() < now && m.set.unwrap() > now);
            assert!(m.progress(now).is_some());
        } else {
            assert!(m.next_rise.unwrap() > now);
        }
    }
}
