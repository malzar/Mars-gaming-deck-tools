//! Iconos del tiempo en SVG para un código WMO de Open-Meteo


/// Tipo de icono para un código WMO de Open-Meteo
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sky {
    Clear,
    Partly,
    Cloudy,
    Fog,
    Rain,
    Snow,
    Storm,
}

pub fn sky(code: u8) -> Sky {
    match code {
        0 => Sky::Clear,
        1 | 2 => Sky::Partly,
        3 => Sky::Cloudy,
        45 | 48 => Sky::Fog,
        51..=67 | 80..=82 => Sky::Rain,
        71..=77 | 85 | 86 => Sky::Snow,
        95..=99 => Sky::Storm,
        _ => Sky::Cloudy,
    }
}

const SUN: &str = "#ffc83d";
const CLOUD: &str = "#d9e1ec";
const DARK_CLOUD: &str = "#aab6c8";
const DROP: &str = "#3fa9f5";

fn sun(cx: f32, cy: f32, r: f32) -> String {
    let rays: String = (0..8)
        .map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_4;
            let (s, c) = a.sin_cos();
            format!(
                "M{:.1} {:.1}L{:.1} {:.1}",
                cx + c * r * 1.45,
                cy + s * r * 1.45,
                cx + c * r * 1.95,
                cy + s * r * 1.95
            )
        })
        .collect();
    format!(
        r##"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{SUN}"/><path d="{rays}" stroke="{SUN}" stroke-width="{w:.1}" stroke-linecap="round"/>"##,
        w = r * 0.32
    )
}

fn moon(cx: f32, cy: f32, r: f32) -> String {
    format!(
        r##"<path d="M{x0:.1} {y0:.1}A{r} {r} 0 1 0 {x1:.1} {y1:.1}A{r2:.1} {r2:.1} 0 0 1 {x0:.1} {y0:.1}z" fill="#f2e6b8"/>"##,
        x0 = cx + r * 0.2,
        y0 = cy - r,
        x1 = cx + r,
        y1 = cy + r * 0.3,
        r2 = r * 0.8
    )
}

/// Nube dentro de una caja de 64×64 desplazada a (x, y) con escala s
fn cloud(x: f32, y: f32, s: f32, color: &str) -> String {
    format!(
        r##"<path transform="translate({x} {y}) scale({s})" d="M18 50h30a11 11 0 0 0 0-22 15 15 0 0 0-28-3 12 12 0 0 0-2 25z" fill="{color}"/>"##
    )
}

/// Icono del tiempo en una caja de `size` px con la esquina en (x, y)
pub fn icon(code: u8, is_day: bool, x: f32, y: f32, size: f32) -> String {
    let s = size / 64.0;
    let at = |px: f32, py: f32| (x + px * s, y + py * s);
    let body = |cx: f32, cy: f32, r: f32| {
        let (cx, cy) = at(cx, cy);
        if is_day { sun(cx, cy, r * s) } else { moon(cx, cy, r * s) }
    };
    match sky(code) {
        Sky::Clear => body(32.0, 32.0, 12.0),
        Sky::Partly => format!("{}{}", body(24.0, 22.0, 9.0), cloud(x + 4.0 * s, y + 6.0 * s, s * 0.95, CLOUD)),
        Sky::Cloudy => format!("{}{}", cloud(x + 10.0 * s, y - 4.0 * s, s * 0.7, DARK_CLOUD), cloud(x, y + 4.0 * s, s, CLOUD)),
        Sky::Fog => {
            let lines: String = [24.0, 34.0, 44.0]
                .iter()
                .enumerate()
                .map(|(i, &ly)| {
                    let (x0, y0) = at(10.0 + i as f32 * 4.0, ly);
                    let (x1, _) = at(54.0 - i as f32 * 4.0, ly);
                    format!("M{x0:.1} {y0:.1}H{x1:.1}")
                })
                .collect();
            format!(r##"<path d="{lines}" stroke="{CLOUD}" stroke-width="{:.1}" stroke-linecap="round"/>"##, 5.0 * s)
        }
        Sky::Rain | Sky::Snow | Sky::Storm => {
            let top = cloud(x, y - 10.0 * s, s, DARK_CLOUD);
            let extra: String = match sky(code) {
                Sky::Rain => [22.0, 33.0, 44.0]
                    .iter()
                    .map(|&dx| {
                        let (x0, y0) = at(dx, 46.0);
                        let (x1, y1) = at(dx - 3.0, 55.0);
                        format!(
                            r##"<path d="M{x0:.1} {y0:.1}L{x1:.1} {y1:.1}" stroke="{DROP}" stroke-width="{:.1}" stroke-linecap="round"/>"##,
                            4.0 * s
                        )
                    })
                    .collect(),
                Sky::Snow => [22.0, 33.0, 44.0]
                    .iter()
                    .map(|&dx| {
                        let (cx, cy) = at(dx, 51.0);
                        format!(r##"<circle cx="{cx:.1}" cy="{cy:.1}" r="{:.1}" fill="#fff"/>"##, 3.2 * s)
                    })
                    .collect(),
                _ => {
                    let p = [(34.0, 40.0), (26.0, 52.0), (33.0, 52.0), (28.0, 62.0)]
                        .iter()
                        .map(|&(px, py)| {
                            let (a, b) = at(px, py);
                            format!("{a:.1} {b:.1}")
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!(r##"<polyline points="{p}" fill="none" stroke="{SUN}" stroke-width="{:.1}" stroke-linejoin="round"/>"##, 4.0 * s)
                }
            };
            format!("{top}{extra}")
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iconos() {
        assert_eq!(sky(61), Sky::Rain);
        for code in [0, 2, 3, 45, 61, 71, 95] {
            assert!(icon(code, true, 0.0, 0.0, 64.0).starts_with('<'));
        }
    }
}
