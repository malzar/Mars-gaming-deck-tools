//! Posición de los planetas con los elementos orbitales de Paul Schlyter
//! («How to compute planetary positions», dominio público). Error de un par
//! de minutos de arco (algo más en Saturno, que no lleva sus correcciones).

use super::RAD;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Planet {
    Mercury,
    Venus,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
}

/// Elementos (N, i, w, a, e, M): valor y cambio por día
type Elems = [(f64, f64); 6];

fn elements(p: Option<Planet>) -> Elems {
    match p {
        None => [(0.0, 0.0), (0.0, 0.0), (282.9404, 4.70935e-5), (1.0, 0.0), (0.016709, -1.151e-9), (356.0470, 0.985_600_258_5)],
        Some(Planet::Mercury) => [(48.3313, 3.24587e-5), (7.0047, 5.00e-8), (29.1241, 1.01444e-5), (0.387098, 0.0), (0.205635, 5.59e-10), (168.6562, 4.092_334_436_8)],
        Some(Planet::Venus) => [(76.6799, 2.46590e-5), (3.3946, 2.75e-8), (54.8910, 1.38374e-5), (0.723330, 0.0), (0.006773, -1.302e-9), (48.0052, 1.602_130_224_4)],
        Some(Planet::Mars) => [(49.5574, 2.11081e-5), (1.8497, -1.78e-8), (286.5016, 2.92961e-5), (1.523688, 0.0), (0.093405, 2.516e-9), (18.6021, 0.524_020_776_6)],
        Some(Planet::Jupiter) => [(100.4542, 2.76854e-5), (1.3030, -1.557e-7), (273.8777, 1.64505e-5), (5.20256, 0.0), (0.048498, 4.469e-9), (19.8950, 0.083_085_300_1)],
        Some(Planet::Saturn) => [(113.6634, 2.38980e-5), (2.4886, -1.081e-7), (339.3939, 2.97661e-5), (9.55475, 0.0), (0.055546, -9.499e-9), (316.9670, 0.033_444_228_2)],
        Some(Planet::Uranus) => [(74.0005, 1.3978e-5), (0.7733, 1.9e-8), (96.6612, 3.0565e-5), (19.18171, -1.55e-8), (0.047318, 7.45e-9), (142.5905, 0.011_725_806)],
        Some(Planet::Neptune) => [(131.7806, 3.0173e-5), (1.7700, -2.55e-7), (272.8461, -6.027e-6), (30.05826, 3.313e-8), (0.008606, 2.15e-9), (260.2471, 0.005_995_147)],
    }
}

/// Posición en su órbita: (x, y, z) heliocéntricos en UA, en la eclíptica
fn heliocentric(p: Option<Planet>, d: f64) -> (f64, f64, f64) {
    let el = elements(p);
    let v = |k: usize| el[k].0 + el[k].1 * d;
    let (n, i, w, a, e, m) = (v(0) * RAD, v(1) * RAD, v(2) * RAD, v(3), v(4), (v(5) % 360.0) * RAD);
    // Ecuación de Kepler
    let mut ea = m + e * m.sin() * (1.0 + e * m.cos());
    for _ in 0..8 {
        ea -= (ea - e * ea.sin() - m) / (1.0 - e * ea.cos());
    }
    let (xv, yv) = (a * (ea.cos() - e), a * (1.0 - e * e).sqrt() * ea.sin());
    let (vv, r) = (yv.atan2(xv), xv.hypot(yv));
    if p.is_none() {
        // El Sol visto desde la Tierra
        return (r * (vv + w).cos(), r * (vv + w).sin(), 0.0);
    }
    let vw = vv + w;
    (
        r * (n.cos() * vw.cos() - n.sin() * vw.sin() * i.cos()),
        r * (n.sin() * vw.cos() + n.cos() * vw.sin() * i.cos()),
        r * vw.sin() * i.sin(),
    )
}

/// (ascensión recta, declinación) en radianes y distancia a la Tierra en UA
pub fn planet_position(p: Planet, t: DateTime<Utc>) -> (f64, f64, f64) {
    // Días desde el 0 de enero de 2000 a las 0 h (el día de Schlyter)
    let d = super::j2000_days(t) + 1.5;
    let (xh, yh, zh) = heliocentric(Some(p), d);
    let (xs, ys, _) = heliocentric(None, d);
    let (xg, yg, zg) = (xh + xs, yh + ys, zh);
    let ecl = (23.4393 - 3.563e-7 * d) * RAD;
    let (xe, ye, ze) = (xg, yg * ecl.cos() - zg * ecl.sin(), yg * ecl.sin() + zg * ecl.cos());
    let ra = ye.atan2(xe).rem_euclid(std::f64::consts::TAU);
    (ra, ze.atan2(xe.hypot(ye)), (xe * xe + ye * ye + ze * ze).sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Ejemplo de Schlyter: 19 de abril de 1990, 0 h UT
    #[test]
    fn ejemplo_de_schlyter() {
        let t = Utc.with_ymd_and_hms(1990, 4, 19, 0, 0, 0).unwrap();
        let (ra, dec, dist) = planet_position(Planet::Mercury, t);
        // Valores de su ejemplo: RA 43,26°, Dec +19,65°, 0,748296 UA
        assert!((ra / RAD - 43.26).abs() < 0.05, "{}", ra / RAD);
        assert!((dec / RAD - 19.65).abs() < 0.05, "{}", dec / RAD);
        assert!((dist - 0.748296).abs() < 1e-4, "{dist}");
        // Júpiter, más o menos a 5 UA
        let (_, _, dj) = planet_position(Planet::Jupiter, t);
        assert!((4.0..6.5).contains(&dj));
    }
}
