//! Dibujo de un temporizador o Pomodoro: anillo en las teclas y barra en las
//! ruedas, del color de la fase

use std::time::{Duration, Instant};

use super::{Kind, settings::TSettings, state::{Phase, State}};
use crate::{
    i18n::{t, tf},
    svg::{ACCENT, MUTED, text},
};

/// Esfera con agujas: la de los minutos y la de los segundos (del color de la
/// fase). En la cuenta atrás marcan lo que queda, así que van hacia atrás.
fn analog(t: Duration, color: &str) -> String {
    let secs = t.as_secs_f32();
    let hand = |angle: f32, len: f32, width: f32, c: &str| {
        let a = (angle - 90.0).to_radians();
        format!(r##"<path d="M72 72L{:.1} {:.1}" stroke="{c}" stroke-width="{width}" stroke-linecap="round"/>"##, 72.0 + a.cos() * len, 72.0 + a.sin() * len)
    };
    let ticks: String = (0..12)
        .map(|i| {
            let a = (i as f32 * 30.0 - 90.0).to_radians();
            let (r0, r1) = if i % 3 == 0 { (52.0, 62.0) } else { (57.0, 62.0) };
            format!("M{:.1} {:.1}L{:.1} {:.1}", 72.0 + a.cos() * r0, 72.0 + a.sin() * r0, 72.0 + a.cos() * r1, 72.0 + a.sin() * r1)
        })
        .collect();
    format!(
        r##"<circle cx="72" cy="72" r="66" fill="none" stroke="#3a4558" stroke-width="4"/><path d="{ticks}" stroke="{MUTED}" stroke-width="4" stroke-linecap="round"/>{}{}<circle cx="72" cy="72" r="5" fill="{color}"/>"##,
        hand((secs / 60.0 % 60.0) * 6.0, 40.0, 7.0, "#fff"),
        hand((secs % 60.0) * 6.0, 56.0, 3.0, color),
    )
}

/// Tiempo grande, con las centésimas pequeñas al lado en el cronómetro por
/// debajo de una hora
fn time_text(x: f32, y: f32, size: f32, color: &str, time: &str, hundredths: Option<u32>) -> String {
    match hundredths {
        Some(cs) => format!(
            r##"<text x="{x}" y="{y}" font-family="sans-serif" font-size="{size}" font-weight="bold" fill="{color}" text-anchor="middle">{}<tspan font-size="{small}">.{cs:02}</tspan></text>"##,
            crate::svg::esc(time),
            small = (size * 0.55).round()
        ),
        None => text(x, y, size, color, true, time),
    }
}

fn clock(d: Duration) -> String {
    let s = d.as_secs() + u64::from(d.subsec_millis() > 0);
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{:02}:{:02}", s / 60, s % 60) }
}

/// Color de la fase
fn color(phase: Phase) -> &'static str {
    match phase {
        Phase::Timer | Phase::Stopwatch => ACCENT,
        Phase::Work => "#ff6b5a",
        Phase::Break | Phase::LongBreak => "#2ecc71",
    }
}

/// Texto de arriba: qué es o en qué fase va
fn label(kind: Kind, st: &State, s: &TSettings) -> String {
    match st.phase {
        Phase::Timer => t("timer.title").to_string(),
        Phase::Stopwatch => t("timer.stopwatch").to_string(),
        Phase::Work if kind == Kind::Pomodoro => tf("timer.work", &[&st.round, &s.rounds.clamp(1, 12)]),
        Phase::Break => t("timer.break").to_string(),
        Phase::LongBreak => t("timer.long_break").to_string(),
        Phase::Work => String::new(),
    }
}

/// Dibujo de un temporizador. `blink` = mitad del parpadeo en la que se apaga.
pub fn body(kind: Kind, st: &State, s: &TSettings, dial: bool, now: Instant, blink: bool) -> String {
    let left = st.remaining(now);
    // Cronómetro: el anillo da una vuelta por minuto
    let frac = if st.phase == Phase::Stopwatch {
        (left.as_secs_f32() % 60.0) / 60.0
    } else if st.total.is_zero() {
        0.0
    } else {
        left.as_secs_f32() / st.total.as_secs_f32()
    };
    let c = color(st.phase);
    let editing = st.is_editing(now);
    let finished = st.finished.is_some();
    // Al acabar: fondo del color de la fase, parpadeando
    let flash = if finished && !blink { format!(r##"<rect width="{w}" height="{h}" fill="{c}" opacity="0.55"/>"##, w = if dial { 200 } else { 144 }, h = if dial { 100 } else { 144 }) } else { String::new() };
    let time = if finished && !st.running() && kind == Kind::Timer { t("timer.done").to_string() } else { clock(left) };
    // Cronómetro: centésimas hasta la primera hora
    let hundredths = (st.phase == Phase::Stopwatch && s.hundredths && left < Duration::from_secs(3600)).then(|| left.subsec_millis() / 10);
    // Modificando: parpadea, salvo mientras se gira la rueda
    let time_color = if editing && blink && !st.is_turning(now) { "#000" } else { "#fff" };
    let status = if editing {
        t("timer.turn_to_change").to_string()
    } else if !st.running() && !finished && !left.is_zero() && (left < st.total || st.phase == Phase::Stopwatch) {
        t("timer.paused").to_string()
    } else {
        label(kind, st, s)
    };
    let status_color = if editing { ACCENT } else if st.running() { c } else { MUTED };
    if dial {
        let bar = format!(
            r##"<rect x="10" y="84" width="180" height="8" rx="4" fill="#1d2a3a"/><rect x="10" y="84" width="{:.1}" height="8" rx="4" fill="{c}"/>"##,
            180.0 * frac
        );
        let size = if time.len() > 5 || hundredths.is_some() { 44.0 } else { 52.0 };
        return format!(
            "{flash}{}{}{bar}",
            text(100.0, 24.0, if editing { 17.0 } else { 20.0 }, status_color, true, &status),
            time_text(100.0, 70.0, size, time_color, &time, hundredths),
        );
    }
    // Tecla con agujas (temporizador y cronómetro): en marcha o sin empezar.
    // En pausa se ve en digital, para leer el tiempo exacto.
    let paused = !st.running() && !finished && !left.is_zero() && (left < st.total || st.phase == Phase::Stopwatch);
    if kind == Kind::Timer && s.is_analog() && !paused && !finished && !editing {
        // Sin centésimas, las agujas saltan de segundo en segundo (menos envíos)
        let t = if s.hundredths { left } else { Duration::from_secs(left.as_secs()) };
        return analog(t, c);
    }
    // Tecla: anillo que se va vaciando
    let r = 62.0;
    let len = 2.0 * std::f32::consts::PI * r;
    let ring = format!(
        r##"<circle cx="72" cy="72" r="{r}" fill="none" stroke="#1d2a3a" stroke-width="9"/><circle cx="72" cy="72" r="{r}" fill="none" stroke="{c}" stroke-width="9" stroke-linecap="round" stroke-dasharray="{:.1} {len:.1}" transform="rotate(-90 72 72)"/>"##,
        len * frac
    );
    let size = if time.len() > 5 || hundredths.is_some() { 28.0 } else { 38.0 };
    // En la tecla el temporizador en marcha no lleva rótulo: ya se ve qué es
    let status = if matches!(st.phase, Phase::Timer | Phase::Stopwatch) && st.running() { String::new() } else { status };
    format!(
        "{flash}{ring}{}{}",
        time_text(72.0, 88.0, size, time_color, &time, hundredths),
        text(72.0, 50.0, 14.0, status_color, true, &status),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::wrap;

    #[test]
    fn reloj() {
        assert_eq!(clock(Duration::from_secs(3725)), "1:02:05");
        assert_eq!(clock(Duration::from_millis(59_200)), "01:00");
    }

    #[test]
    #[ignore]
    fn muestras_temporizador() {
        let dir = std::path::Path::new("target/widgets");
        std::fs::create_dir_all(dir).unwrap();
        let s = TSettings::default();
        let t0 = Instant::now();
        let mut out: Vec<(&str, String, bool)> = vec![];
        let mut a = State::new(Kind::Timer, &s);
        a.toggle(t0);
        out.push(("temp-marcha", body(Kind::Timer, &a, &s, false, t0 + Duration::from_secs(110), false), false));
        out.push(("temp-rueda-marcha", body(Kind::Timer, &a, &s, true, t0 + Duration::from_secs(110), false), true));
        let mut p = State::new(Kind::Pomodoro, &s);
        p.round = 2;
        p.toggle(t0);
        out.push(("pomo-trabajo", body(Kind::Pomodoro, &p, &s, false, t0 + Duration::from_secs(300), false), false));
        out.push(("pomo-rueda-trabajo", body(Kind::Pomodoro, &p, &s, true, t0 + Duration::from_secs(300), false), true));
        let mut b = State::new(Kind::Pomodoro, &s);
        b.next_phase(&s, t0, true);
        out.push(("pomo-descanso", body(Kind::Pomodoro, &b, &s, false, t0 + Duration::from_secs(60), false), false));
        let mut e = State::new(Kind::Timer, &s);
        e.editing = Some(t0 + Duration::from_secs(5));
        out.push(("temp-rueda-modificar", body(Kind::Timer, &e, &s, true, t0, false), true));
        let mut f = State::new(Kind::Timer, &s);
        f.toggle(t0);
        f.tick(Kind::Timer, &s, t0 + Duration::from_secs(301));
        out.push(("temp-fin", body(Kind::Timer, &f, &s, false, t0 + Duration::from_secs(302), false), false));
        let mut q = State::new(Kind::Timer, &s);
        q.toggle(t0);
        q.toggle(t0 + Duration::from_secs(40));
        out.push(("temp-pausa", body(Kind::Timer, &q, &s, false, t0 + Duration::from_secs(50), false), false));
        for (name, b, dial) in out {
            std::fs::write(dir.join(format!("{name}.svg")), wrap(dial, &b)).unwrap();
        }
    }
}
