//! «Lluvia»: la lluvia prevista desde ahora y durante el tiempo elegido (de
//! 15 minutos a 2 días), en barras. Hasta 6 horas va por cuartos de hora;
//! más allá, por horas. Si no se espera lluvia en ese tiempo, la vista no se
//! enseña.

use super::Ctx;
use crate::{
    i18n::{t, tf},
    svg::{ACCENT, MUTED, text},
    weather::Weather,
};

/// Barras como mucho
const MAX_BARS: u32 = 12;
pub const MIN_SPAN: u32 = 15;
pub const MAX_SPAN: u32 = 2 * 24 * 60;

/// Lluvia (l/m²) de cada barra para los próximos `span` minutos, y cuántos
/// minutos ocupa cada una
pub fn bars(w: &Weather, span: u32) -> (Vec<f32>, u32) {
    let span = span.clamp(MIN_SPAN, MAX_SPAN);
    if span <= 6 * 60 && !w.quarters.is_empty() {
        let per = (span / 15).div_ceil(MAX_BARS).max(1) as usize;
        let n = (span / 15) as usize;
        let q: Vec<f32> = w.quarters.iter().take(n).map(|q| q.mm).collect();
        (q.chunks(per).map(|c| c.iter().sum()).collect(), per as u32 * 15)
    } else {
        let per = (span / 60).max(1).div_ceil(MAX_BARS).max(1) as usize;
        let n = (span / 60).max(1) as usize;
        let h: Vec<f32> = w.hours.iter().take(n).map(|h| h.mm).collect();
        (h.chunks(per).map(|c| c.iter().sum()).collect(), per as u32 * 60)
    }
}

/// Si se espera lluvia en los próximos `span` minutos
pub fn coming(w: &Weather, span: u32) -> bool {
    bars(w, span).0.iter().any(|&mm| mm >= 0.1)
}

/// «15 min», «6 h», «2 días»
fn span_text(span: u32) -> String {
    match span {
        s if s < 60 => format!("{s} min"),
        s if s < 24 * 60 => format!("{} h", s / 60),
        s if s == 24 * 60 => t("weather.one_day").into(),
        s => tf("weather.days", &[&(s / (24 * 60))]),
    }
}

pub fn draw(c: &Ctx, dial: bool) -> String {
    let (values, per) = bars(c.w, c.rain_span);
    let (x0, w, base, h, label_y) = if dial { (12.0, 176.0, 78.0, 50.0, 96.0) } else { (12.0, 120.0, 108.0, 66.0, 134.0) };
    let head = tf("weather.rain_span", &[&span_text(c.rain_span)]);
    let title = if dial { text(100.0, 22.0, 16.0, MUTED, false, &head) } else { text(72.0, 28.0, 17.0, MUTED, false, &head) };
    if values.is_empty() {
        let (cx, cy) = if dial { (100.0, 60.0) } else { (72.0, 80.0) };
        return format!("{title}{}", text(cx, cy, 18.0, MUTED, false, t("weather.no_data")));
    }
    // La barra llena: 2 l/m² cada 15 minutos
    let full = 2.0 * per as f32 / 15.0;
    let bw = w / values.len() as f32;
    let mut out = String::new();
    for (i, mm) in values.iter().enumerate() {
        let bh = if *mm > 0.0 { (h * (mm / full).clamp(0.0, 1.0)).max(4.0) } else { 2.0 };
        let color = if *mm > 0.0 { ACCENT } else { "#2a3442" };
        out += &format!(r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{bh:.1}" rx="2" fill="{color}"/>"##, x0 + i as f32 * bw + 1.5, base - bh, (bw - 3.0).max(1.0));
    }
    let total: f32 = values.iter().sum();
    let foot = if total < 0.1 { t("weather.no_rain").to_string() } else { format!("{total:.1} l/m²") };
    format!("{title}{out}{}", text(if dial { 100.0 } else { 72.0 }, label_y, if dial { 15.0 } else { 17.0 }, if total < 0.1 { MUTED } else { ACCENT }, false, &foot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weather::{Hour, Quarter};

    #[test]
    fn barras_segun_el_tiempo() {
        let w = Weather {
            quarters: (0..24).map(|i| Quarter { time: String::new(), mm: if i == 3 { 1.0 } else { 0.0 } }).collect(),
            hours: (0..49).map(|i| Hour { mm: if i == 30 { 2.0 } else { 0.0 }, ..Default::default() }).collect(),
            ..Default::default()
        };
        assert_eq!(bars(&w, 15), (vec![0.0], 15));
        assert_eq!(bars(&w, 120).0.len(), 8);
        assert_eq!(bars(&w, 360), (bars(&w, 360).0.clone(), 30));
        assert_eq!(bars(&w, 360).0.len(), 12);
        assert!(!coming(&w, 45) && coming(&w, 60));
        let (d, per) = bars(&w, MAX_SPAN);
        assert_eq!((d.len(), per), (12, 240));
        assert!(coming(&w, MAX_SPAN) && !coming(&w, 12 * 60 + 1));
        assert_eq!(span_text(15), "15 min");
        assert_eq!(span_text(360), "6 h");
    }
}
