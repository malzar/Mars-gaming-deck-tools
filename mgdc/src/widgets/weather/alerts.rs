//! Avisos del tiempo: lluvia en un rato y meteorología extrema, oficiales
//! (del organismo del país) o calculados con la previsión, y su dibujo.

use crate::{
    i18n::{t, tf},
    svg::{MUTED, clip, text},
    weather::{self, Hazard, Level, Warning, Weather},
    widgets::settings::WSettings,
};

/// Color del borde: lluvia o nivel
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tone {
    Rain,
    Yellow,
    Orange,
    Red,
}

impl Tone {
    fn color(self) -> &'static str {
        match self {
            Tone::Rain => "#4aa3ff",
            Tone::Yellow => "#f5c542",
            Tone::Orange => "#ff8c1a",
            Tone::Red => "#e5483b",
        }
    }
}

impl From<Level> for Tone {
    fn from(l: Level) -> Self {
        match l {
            Level::Yellow => Tone::Yellow,
            Level::Orange => Tone::Orange,
            Level::Red => Tone::Red,
        }
    }
}

/// Un aviso para enseñar
#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub hazard: Hazard,
    pub tone: Tone,
    /// «Lluvia», «Viento»…
    pub title: String,
    /// «en 25 min», «Aviso naranja», «Hasta 41°»…
    pub detail: String,
    /// «hasta las 21:00» (solo en la rueda)
    pub extra: String,
    /// Para no repetir el sonido ni volver a enseñarlo si se quitó
    pub id: String,
}

fn hazard_name(h: Hazard) -> &'static str {
    t(match h {
        Hazard::Rain => "alert.rain",
        Hazard::Heat => "alert.heat",
        Hazard::Cold => "alert.cold",
        Hazard::Wind => "alert.wind",
        Hazard::Storm => "alert.storm",
        Hazard::Snow => "alert.snow",
        Hazard::Other => "alert.other",
    })
}

fn hour_of(time: &str) -> &str {
    time.get(11..16).unwrap_or("")
}

/// Minutos de `from` a `to` ("2026-11-16T19:15")
fn minutes(from: &str, to: &str) -> i64 {
    let p = |s: &str| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").ok();
    match (p(from), p(to)) {
        (Some(a), Some(b)) => (b - a).num_minutes(),
        _ => 0,
    }
}

/// ¿Va a llover pronto? Primero la lluvia cada 15 minutos; si no hay, la
/// probabilidad por horas
pub fn rain(w: &Weather, s: &WSettings) -> Option<Alert> {
    if !s.alert_rain {
        return None;
    }
    let within = s.rain_minutes.clamp(15, 360) as i64;
    let start = w.quarters.iter().find(|q| q.mm >= 0.1 && minutes(&w.time, &q.time) <= within).map(|q| q.time.clone()).or_else(|| {
        w.hours.iter().find(|h| h.rain >= s.rain_prob && minutes(&w.time, &h.time) <= within).map(|h| h.time.clone())
    })?;
    let m = minutes(&w.time, &start).max(0);
    let detail = if m < 10 { t("alert.rain_now").to_string() } else { tf("alert.in_min", &[&m]) };
    Some(Alert { hazard: Hazard::Rain, tone: Tone::Rain, title: hazard_name(Hazard::Rain).into(), detail, extra: String::new(), id: format!("rain-{}", start.get(..13).unwrap_or("")) })
}

/// Avisos calculados con la previsión de las próximas horas
pub fn computed(w: &Weather, s: &WSettings) -> Vec<Alert> {
    if !s.alert_calc {
        return vec![];
    }
    let day = w.time.get(..10).unwrap_or("");
    let next = |n: usize| w.hours.iter().take(n);
    let mut out = vec![];
    let mut push = |hazard: Hazard, tone: Tone, detail: String, extra: String| {
        out.push(Alert { hazard, tone, title: hazard_name(hazard).into(), detail, extra, id: format!("{hazard:?}-{tone:?}-{day}") });
    };
    let step = |v: f32, limit: f32, unit: f32| if v >= limit + 2.0 * unit { Tone::Red } else if v >= limit + unit { Tone::Orange } else { Tone::Yellow };
    if let Some(h) = next(6).max_by(|a, b| a.gusts.total_cmp(&b.gusts)).filter(|h| h.gusts >= s.gust_kmh) {
        push(Hazard::Wind, step(h.gusts, s.gust_kmh, s.gust_kmh * 0.3), tf("alert.gusts", &[&(h.gusts.round() as i32)]), tf("alert.at", &[&hour_of(&h.time)]));
    }
    if let Some(h) = next(12).max_by(|a, b| a.temp.total_cmp(&b.temp)).filter(|h| h.temp >= s.heat_c) {
        push(Hazard::Heat, step(h.temp, s.heat_c, 3.0), tf("alert.up_to", &[&(h.temp.round() as i32)]), tf("alert.at", &[&hour_of(&h.time)]));
    }
    if let Some(h) = next(12).min_by(|a, b| a.temp.total_cmp(&b.temp)).filter(|h| h.temp <= s.cold_c) {
        push(Hazard::Cold, step(-h.temp, -s.cold_c, 3.0), tf("alert.down_to", &[&(h.temp.round() as i32)]), tf("alert.at", &[&hour_of(&h.time)]));
    }
    if s.alert_storm && let Some(h) = next(6).find(|h| (95..=99).contains(&h.code)) {
        push(Hazard::Storm, Tone::Orange, tf("alert.at", &[&hour_of(&h.time)]), String::new());
    }
    if s.alert_snow && let Some(h) = next(12).find(|h| matches!(h.code, 71..=77 | 85 | 86)) {
        push(Hazard::Snow, Tone::Yellow, tf("alert.at", &[&hour_of(&h.time)]), String::new());
    }
    out
}

/// Un aviso oficial, para enseñar
pub fn from_official(w: &Warning) -> Alert {
    let level = t(match w.level {
        Level::Yellow => "alert.yellow",
        Level::Orange => "alert.orange",
        Level::Red => "alert.red",
    });
    let extra = w.expires.map(|e| tf("alert.until", &[&e.with_timezone(&chrono::Local).format("%H:%M")])).unwrap_or_default();
    Alert { hazard: w.hazard, tone: w.level.into(), title: hazard_name(w.hazard).into(), detail: level.into(), extra, id: w.id.clone() }
}

/// Todos los avisos, de más a menos importante: los oficiales mandan sobre
/// los calculados del mismo tipo
pub fn all(w: &Weather, s: &WSettings, official: &[Warning]) -> Vec<Alert> {
    let mut out: Vec<Alert> = if s.alert_official { official.iter().map(from_official).collect() } else { vec![] };
    for a in computed(w, s).into_iter().chain(rain(w, s)) {
        if !out.iter().any(|o| o.hazard == a.hazard) {
            out.push(a);
        }
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.tone));
    out
}

/// El sonido elegido para cada tipo de aviso ("" = ninguno)
pub fn sound_of(s: &WSettings, h: Hazard) -> &str {
    match h {
        Hazard::Rain => &s.snd_rain,
        Hazard::Heat => &s.snd_heat,
        Hazard::Cold => &s.snd_cold,
        Hazard::Wind => &s.snd_wind,
        Hazard::Storm => &s.snd_storm,
        Hazard::Snow => &s.snd_snow,
        Hazard::Other => &s.snd_other,
    }
}

/// Icono de cada tipo de aviso (en x, y con lado `size`)
fn icon(h: Hazard, color: &str, x: f32, y: f32, size: f32) -> String {
    let code = match h {
        Hazard::Rain => Some(63),
        Hazard::Storm => Some(95),
        Hazard::Snow | Hazard::Cold => Some(73),
        Hazard::Heat => Some(0),
        _ => None,
    };
    if let Some(c) = code {
        return weather::icon(c, true, x, y, size);
    }
    let s = size / 64.0;
    let p = |px: f32, py: f32| format!("{:.1} {:.1}", x + px * s, y + py * s);
    if h == Hazard::Wind {
        format!(
            r##"<g fill="none" stroke="{color}" stroke-width="{w:.1}" stroke-linecap="round"><path d="M{} L{} Q{} {}"/><path d="M{} L{} Q{} {}"/><path d="M{} L{}"/></g>"##,
            p(6.0, 22.0), p(44.0, 22.0), p(56.0, 22.0), p(50.0, 10.0),
            p(6.0, 36.0), p(54.0, 36.0), p(64.0, 40.0), p(56.0, 52.0),
            p(6.0, 50.0), p(32.0, 50.0),
            w = 5.0 * s
        )
    } else {
        // Aviso genérico: un triángulo con «!»
        format!(
            r##"<path d="M{} L{} L{} Z" fill="none" stroke="{color}" stroke-width="{w:.1}" stroke-linejoin="round"/><path d="M{} L{}" stroke="{color}" stroke-width="{w:.1}" stroke-linecap="round"/>"##,
            p(32.0, 6.0), p(60.0, 56.0), p(4.0, 56.0), p(32.0, 24.0), p(32.0, 40.0),
            w = 5.0 * s
        )
    }
}

/// Vista «Avisos»: la lista de los que hay ahora (si no hay, la vista no se
/// enseña)
pub fn list_view(c: &super::Ctx, dial: bool) -> String {
    let (head, rows, x, y0, step, w) = if dial { (text(100.0, 20.0, 15.0, MUTED, false, t("alert.list")), 2, 14.0, 46.0, 34.0, 22) } else { (text(72.0, 24.0, 15.0, MUTED, false, t("alert.list")), 3, 12.0, 52.0, 38.0, 13) };
    if c.alerts.is_empty() {
        return format!("{head}{}", text(if dial { 100.0 } else { 72.0 }, if dial { 62.0 } else { 80.0 }, 17.0, MUTED, false, t("alert.none")));
    }
    let mut out = head;
    for (i, a) in c.alerts.iter().take(rows).enumerate() {
        let y = y0 + i as f32 * step;
        let col = a.tone.color();
        out += &format!(
            r##"<circle cx="{:.1}" cy="{:.1}" r="5" fill="{col}"/><text x="{:.1}" y="{:.1}" font-family="sans-serif" font-size="15" font-weight="bold" fill="#fff">{}</text><text x="{:.1}" y="{:.1}" font-family="sans-serif" font-size="13" fill="{col}">{}</text>"##,
            x + 4.0, y - 5.0, x + 14.0, y,
            crate::svg::esc(&clip(&a.title, w)),
            x + 14.0, y + 15.0,
            crate::svg::esc(&clip(&a.detail, w + 4))
        );
    }
    if c.alerts.len() > rows {
        out += &text(if dial { 186.0 } else { 132.0 }, if dial { 94.0 } else { 138.0 }, 12.0, MUTED, false, &format!("+{}", c.alerts.len() - rows));
    }
    out
}

/// Dibujo de un aviso (cuerpo sin el `<svg>`)
pub fn draw(a: &Alert, dial: bool) -> String {
    let c = a.tone.color();
    if dial {
        let extra = if a.extra.is_empty() { String::new() } else { text(122.0, 88.0, 14.0, MUTED, false, &clip(&a.extra, 20)) };
        return format!(
            r##"<rect x="2" y="2" width="196" height="96" rx="4" fill="none" stroke="{c}" stroke-width="3"/>{}{}{}{extra}"##,
            icon(a.hazard, c, 10.0, 18.0, 64.0),
            text(132.0, 40.0, 21.0, "#fff", true, &clip(&a.title, 12)),
            text(132.0, 66.0, 17.0, c, false, &clip(&a.detail, 15)),
        );
    }
    format!(
        r##"<rect x="3" y="3" width="138" height="138" rx="12" fill="none" stroke="{c}" stroke-width="6"/>{}{}{}"##,
        icon(a.hazard, c, 38.0, 12.0, 68.0),
        text(72.0, 102.0, 20.0, "#fff", true, &clip(&a.title, 11)),
        text(72.0, 126.0, 17.0, c, false, &clip(&a.detail, 13)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weather::{Hour, Quarter};

    fn w() -> Weather {
        let mut w = Weather { time: "2026-11-19T14:00".into(), ..Default::default() };
        w.hours = (14..26)
            .map(|h| Hour { time: format!("2026-11-{}T{:02}:00", 19 + h / 24, h % 24), temp: 20.0, code: 1, rain: 0, gusts: 10.0, mm: 0.0 })
            .collect();
        w
    }

    #[test]
    fn lluvia_pronto() {
        let s = WSettings::default();
        let mut x = w();
        assert!(rain(&x, &s).is_none());
        x.quarters = vec![Quarter { time: "2026-11-19T14:00".into(), mm: 0.0 }, Quarter { time: "2026-11-19T14:30".into(), mm: 0.6 }];
        let a = rain(&x, &s).unwrap();
        assert_eq!(a.detail, tf("alert.in_min", &[&30]));
        // Por horas, si no hay cuartos de hora con lluvia
        x.quarters.clear();
        x.hours[1].rain = 70;
        assert_eq!(rain(&x, &s).unwrap().detail, tf("alert.in_min", &[&60]));
        x.hours[1].rain = 30;
        assert!(rain(&x, &s).is_none());
    }

    #[test]
    fn extremos_calculados() {
        let s = WSettings::default();
        let mut x = w();
        assert!(computed(&x, &s).is_empty());
        x.hours[2].gusts = 95.0;
        x.hours[5].temp = 44.5;
        x.hours[3].code = 95;
        let a = computed(&x, &s);
        let wind = a.iter().find(|a| a.hazard == Hazard::Wind).unwrap();
        assert_eq!(wind.tone, Tone::Orange);
        assert_eq!(a.iter().find(|a| a.hazard == Hazard::Heat).unwrap().tone, Tone::Red);
        assert!(a.iter().any(|a| a.hazard == Hazard::Storm));
        // Un aviso oficial del mismo tipo manda
        let off = Warning { hazard: Hazard::Wind, level: Level::Yellow, event: "x".into(), expires: None, id: "o".into() };
        let list = all(&x, &s, &[off]);
        assert_eq!(list.iter().filter(|a| a.hazard == Hazard::Wind).count(), 1);
        assert_eq!(list.iter().find(|a| a.hazard == Hazard::Wind).unwrap().id, "o");
        assert_eq!(list[0].tone, Tone::Red);
        for a in &list {
            for dial in [false, true] {
                let svg = crate::svg::wrap(dial, &draw(a, dial));
                assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
            }
        }
    }
}

#[cfg(test)]
#[test]
#[ignore]
fn volcar_avisos() {
    use crate::weather::Quarter;
    let dir = std::env::var("OUT").unwrap();
    let a = |hazard, tone, d: &str, e: &str| Alert { hazard, tone, title: hazard_name(hazard).into(), detail: d.into(), extra: e.into(), id: String::new() };
    let list = [a(Hazard::Rain, Tone::Rain, "en 25 min", ""), a(Hazard::Wind, Tone::Orange, "Aviso naranja", "hasta las 21:00"), a(Hazard::Heat, Tone::Red, "hasta 42°", "a las 16:00"), a(Hazard::Snow, Tone::Yellow, "a las 03:00", "")];
    let mut i = 0;
    for x in &list {
        for dial in [false, true] {
            std::fs::write(format!("{dir}/m{i}.svg"), crate::svg::wrap(dial, &draw(x, dial))).unwrap();
            i += 1;
        }
    }
    let mut w = Weather { time: "2026-11-19T14:00".into(), ..Default::default() };
    w.quarters = [0.0, 0.0, 0.3, 1.2, 2.5, 0.8, 0.1, 0.0].iter().enumerate().map(|(k, &mm)| Quarter { time: format!("2026-11-19T14:{:02}", k * 15 % 60), mm }).collect();
    let c = super::Ctx { w: &w, city: "Madrid", sky: None, rain_span: 120, alerts: &[] };
    for dial in [false, true] {
        std::fs::write(format!("{dir}/m{i}.svg"), crate::svg::wrap(dial, &super::rain::draw(&c, dial))).unwrap();
        i += 1;
    }
}
