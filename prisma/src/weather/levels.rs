//! Niveles para enseñar: UV, calidad del aire, polen y el viento en palabras

pub fn aqi_level(v: f32) -> (&'static str, &'static str) {
    match v {
        x if x < 20.0 => ("air.good", "#2ecc71"),
        x if x < 40.0 => ("air.fair", "#a3d65c"),
        x if x < 60.0 => ("air.moderate", "#ffc83d"),
        x if x < 80.0 => ("air.poor", "#ff8a3d"),
        x if x < 100.0 => ("air.very_poor", "#ff5a5a"),
        _ => ("air.extreme", "#b05cff"),
    }
}

/// Índice UV: claves del nivel y del consejo, y color
pub fn uv_level(v: f32) -> (&'static str, &'static str, &'static str) {
    match v.round() as i32 {
        ..=2 => ("level.low", "#2ecc71", "uv.tip.none"),
        3..=5 => ("level.moderate", "#ffc83d", "uv.tip.glasses"),
        6..=7 => ("level.high", "#ff8a3d", "uv.tip.shade"),
        8..=10 => ("level.very_high", "#ff5a5a", "uv.tip.midday"),
        _ => ("level.extreme", "#b05cff", "uv.tip.stay_in"),
    }
}

/// Polen: clave del nivel y color (umbrales aproximados en granos/m³; la ambrosía y la
/// artemisa dan alergia con menos)
pub fn pollen_level(kind: &str, v: f32) -> (&'static str, &'static str) {
    let (a, b, c) = if matches!(kind, "pollen.ragweed" | "pollen.mugwort") { (5.0, 20.0, 50.0) } else { (10.0, 50.0, 200.0) };
    match v {
        x if x < a => ("level.low", "#2ecc71"),
        x if x < b => ("level.moderate", "#ffc83d"),
        x if x < c => ("level.high", "#ff8a3d"),
        _ => ("level.very_high", "#ff5a5a"),
    }
}

/// Clave del punto cardinal del que sopla el viento
pub fn compass(deg: f32) -> &'static str {
    const P: [&str; 8] = ["dir.n", "dir.ne", "dir.e", "dir.se", "dir.s", "dir.sw", "dir.w", "dir.nw"];
    P[(((deg.rem_euclid(360.0)) + 22.5) / 45.0) as usize % 8]
}

/// Clave del viento en palabras (escala de Beaufort resumida)
pub fn wind_word(kmh: f32) -> &'static str {
    match kmh {
        x if x < 2.0 => "wind.calm",
        x if x < 12.0 => "wind.light",
        x if x < 29.0 => "wind.moderate",
        x if x < 50.0 => "wind.strong",
        x if x < 75.0 => "wind.very_strong",
        _ => "wind.gale",
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn viento() {
        assert_eq!(super::compass(200.0), "dir.s");
        assert_eq!(super::compass(350.0), "dir.n");
    }
}
