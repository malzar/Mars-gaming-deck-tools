//! El cielo: dónde está el Sol, la Luna, los planetas o una estrella, si se
//! ven, a qué hora salen y se ponen, y las horas de luz del día. Todo se
//! calcula aquí mismo, sin internet.
//!
//! - `sun`: salida, puesta, mediodía solar y fases de luz (crepúsculos, hora
//!   dorada y azul)
//! - `planets`: posición de los planetas (elementos orbitales de Paul
//!   Schlyter, error de un par de minutos de arco: de sobra para saber si se
//!   ven y cuándo salen)
//! - Aquí: la altura sobre el horizonte, salidas y puestas de cualquier
//!   astro, y la constelación en la que está

mod planets;
pub mod sun;

use chrono::{DateTime, Duration, Utc};

pub use planets::{Planet, planet_position};

pub const RAD: f64 = std::f64::consts::PI / 180.0;

/// Días desde el 1 de enero de 2000 a mediodía (J2000)
pub fn j2000_days(t: DateTime<Utc>) -> f64 {
    t.timestamp_millis() as f64 / 86_400_000.0 + 2_440_587.5 - 2_451_545.0
}

/// Altura sobre el horizonte (grados) de un astro en (ascensión recta,
/// declinación), en radianes
pub fn altitude(ra: f64, dec: f64, t: DateTime<Utc>, lat: f64, lon: f64) -> f64 {
    let d = j2000_days(t);
    let sidereal = RAD * (280.16 + 360.985_623_5 * d) + RAD * lon;
    let h = sidereal - ra;
    let phi = RAD * lat;
    (phi.sin() * dec.sin() + phi.cos() * dec.cos() * h.cos()).asin() / RAD
}

/// Salida y puesta de un astro (las próximas, en las siguientes ~26 h), y
/// si nunca se pone o nunca sale en ese tiempo
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiseSet {
    pub rise: Option<DateTime<Utc>>,
    pub set: Option<DateTime<Utc>>,
    /// Arriba todo el rato (estrellas circumpolares)
    pub always_up: bool,
    /// Abajo todo el rato
    pub never_up: bool,
}

/// Busca cuándo la altura (`alt(t) - h0`) cruza cero, de 10 en 10 minutos
pub fn rise_set(now: DateTime<Utc>, h0: f64, alt: impl Fn(DateTime<Utc>) -> f64) -> RiseSet {
    let step = Duration::minutes(10);
    let (mut rise, mut set) = (None, None);
    let mut t = now;
    let mut a = alt(t) - h0;
    let (mut up, mut down) = (a >= 0.0, a < 0.0);
    for _ in 0..(26 * 6) {
        let t2 = t + step;
        let a2 = alt(t2) - h0;
        up |= a2 >= 0.0;
        down |= a2 < 0.0;
        if (a < 0.0) != (a2 < 0.0) {
            let k = -a / (a2 - a);
            let at = t + Duration::milliseconds((step.num_milliseconds() as f64 * k) as i64);
            if a2 >= 0.0 { rise.get_or_insert(at) } else { set.get_or_insert(at) };
        }
        if rise.is_some() && set.is_some() {
            break;
        }
        t = t2;
        a = a2;
    }
    RiseSet { rise, set, always_up: !down, never_up: !up }
}

/// La constelación donde está un punto del cielo: la de la estrella
/// brillante (magnitud ≤ 4,5) más cercana del catálogo. Es una aproximación
/// (los límites oficiales no se siguen al detalle), suficiente para saber
/// dónde buscar.
pub fn constellation(ra: f64, dec: f64) -> &'static str {
    crate::stars::STARS
        .iter()
        .filter(|s| s.mag <= 4.5)
        .min_by(|a, b| sep(ra, dec, a).total_cmp(&sep(ra, dec, b)))
        .map_or("", |s| s.con)
}

/// Distancia angular (radianes) de un punto a una estrella
fn sep(ra: f64, dec: f64, s: &crate::stars::Star) -> f64 {
    let (r2, d2) = (s.ra * RAD, s.dec * RAD);
    (dec.sin() * d2.sin() + dec.cos() * d2.cos() * (ra - r2).cos()).clamp(-1.0, 1.0).acos()
}

/// Una estrella del catálogo, por su nombre o designación
pub fn find_star(name: &str) -> Option<&'static crate::stars::Star> {
    let n = name.trim();
    crate::stars::STARS.iter().find(|s| s.name.eq_ignore_ascii_case(n) || s.bayer == n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn vega_desde_madrid() {
        let vega = find_star("Vega").unwrap();
        // 20 de noviembre de 2026, 19:00 UTC: Vega al oeste, alta
        let t = Utc.with_ymd_and_hms(2026, 11, 20, 19, 0, 0).unwrap();
        let alt = altitude(vega.ra * RAD, vega.dec * RAD, t, 40.42, -3.70);
        assert!((30.0..60.0).contains(&alt), "{alt}");
        // Polaris no se pone desde Madrid
        let pol = find_star("Polaris").unwrap();
        let rs = rise_set(t, -0.5667, |x| altitude(pol.ra * RAD, pol.dec * RAD, x, 40.42, -3.70));
        assert!(rs.always_up && rs.rise.is_none());
        // Vega se pone y sale en un día
        let rs = rise_set(t, -0.5667, |x| altitude(vega.ra * RAD, vega.dec * RAD, x, 40.42, -3.70));
        assert!(rs.rise.is_some() && rs.set.is_some());
        assert_eq!(constellation(vega.ra * RAD, vega.dec * RAD), "Lyra");
    }
}
