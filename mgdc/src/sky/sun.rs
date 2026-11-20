//! Las horas del Sol de un día: salida, puesta, mediodía solar y las fases
//! de luz. Fórmulas de SunCalc (BSD de 2 cláusulas, ver THIRD_PARTY.md).

use super::RAD;
use chrono::{DateTime, Duration, Utc};
use std::f64::consts::PI;

const J0: f64 = 0.0009;
const E: f64 = RAD * 23.4397;

/// Cuándo pasa el Sol por una altura (grados) por la mañana y por la tarde
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pair {
    pub morning: Option<DateTime<Utc>>,
    pub evening: Option<DateTime<Utc>>,
}

/// Las horas del Sol de un día
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunDay {
    /// Salida y puesta (borde superior, con la refracción: −0,833°)
    pub rise_set: Pair,
    /// Mediodía solar (el Sol más alto)
    pub noon: DateTime<Utc>,
    /// Crepúsculo civil (−6°), náutico (−12°) y astronómico (−18°)
    pub civil: Pair,
    pub nautical: Pair,
    pub astro: Pair,
    /// Fin y principio de la hora dorada (+6°) y frontera dorada/azul (−4°)
    pub golden: Pair,
    pub blue: Pair,
}

fn from_julian(j: f64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(((j + 0.5 - 2_440_588.0) * 86_400_000.0) as i64).unwrap_or_default()
}

/// Las horas del Sol del día que contiene `day` (en tiempo universal; para el
/// día local, pasar su mediodía)
pub fn sun_day(day: DateTime<Utc>, lat: f64, lon: f64) -> SunDay {
    let lw = RAD * -lon;
    let phi = RAD * lat;
    let d = super::j2000_days(day);
    let n = (d - J0 - lw / (2.0 * PI)).round();
    let ds = J0 + lw / (2.0 * PI) + n;
    let m = RAD * (357.5291 + 0.985_600_28 * ds);
    let c = RAD * (1.9148 * m.sin() + 0.02 * (2.0 * m).sin() + 0.0003 * (3.0 * m).sin());
    let l = m + c + RAD * 102.9372 + PI;
    let dec = (E.sin() * l.sin()).asin();
    let transit = |x: f64| 2_451_545.0 + x + 0.0053 * m.sin() - 0.0069 * (2.0 * l).sin();
    let j_noon = transit(ds);
    let pair = |h: f64| {
        let w = ((RAD * h).sin() - phi.sin() * dec.sin()) / (phi.cos() * dec.cos());
        if !(-1.0..=1.0).contains(&w) {
            return Pair::default();
        }
        let j_set = transit(J0 + (w.acos() + lw) / (2.0 * PI) + n);
        Pair { morning: Some(from_julian(j_noon - (j_set - j_noon))), evening: Some(from_julian(j_set)) }
    };
    SunDay { rise_set: pair(-0.833), noon: from_julian(j_noon), civil: pair(-6.0), nautical: pair(-12.0), astro: pair(-18.0), golden: pair(6.0), blue: pair(-4.0) }
}

impl SunDay {
    /// Horas de luz (de la salida a la puesta)
    pub fn day_length(&self) -> Option<Duration> {
        Some(self.rise_set.evening? - self.rise_set.morning?)
    }
}

/// Las fases de luz del día, en orden
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Night,
    Astronomical,
    Nautical,
    Blue,
    Golden,
    Day,
}

/// Tramos del día: (fase, desde, hasta), de medianoche a medianoche
pub fn light_spans(s: &SunDay) -> Vec<(Light, DateTime<Utc>, DateTime<Utc>)> {
    use Light::*;
    // Mañana: noche → astronómico → náutico → (civil) azul → dorada → día
    let marks = [
        (s.astro.morning, Astronomical),
        (s.nautical.morning, Nautical),
        (s.civil.morning, Blue),
        (s.blue.morning, Golden),
        (s.golden.morning, Day),
        (s.golden.evening, Golden),
        (s.blue.evening, Blue),
        (s.civil.evening, Nautical),
        (s.nautical.evening, Astronomical),
        (s.astro.evening, Night),
    ];
    let start = s.noon - Duration::hours(12);
    let end = s.noon + Duration::hours(12);
    let mut out = vec![];
    let mut cur = (Night, start);
    // Sin noche cerrada (verano en el norte), se empieza en lo que toque
    if s.astro.morning.is_none() {
        cur.0 = if s.nautical.morning.is_none() { Blue } else { Astronomical };
    }
    for (t, next) in marks {
        if let Some(t) = t {
            if t > cur.1 {
                out.push((cur.0, cur.1, t));
            }
            cur = (next, t);
        }
    }
    out.push((cur.0, cur.1, end));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn madrid_en_noviembre() {
        // 20 de noviembre de 2026 en Madrid: sale ~07:10 UTC, se pone ~16:55 UTC
        let s = sun_day(Utc.with_ymd_and_hms(2026, 11, 20, 12, 0, 0).unwrap(), 40.42, -3.70);
        let rise = s.rise_set.morning.unwrap();
        let set = s.rise_set.evening.unwrap();
        assert_eq!(rise.format("%H").to_string(), "07");
        assert_eq!(set.format("%H").to_string(), "16");
        let len = s.day_length().unwrap().num_minutes();
        assert!((560..600).contains(&len), "{len}");
        // Las fases van en orden y sin huecos
        let spans = light_spans(&s);
        assert!(spans.windows(2).all(|w| w[0].2 == w[1].1));
        assert!(spans.iter().any(|x| x.0 == Light::Golden));
        assert!(s.civil.morning.unwrap() < rise && s.golden.evening.unwrap() < set);
    }
}
