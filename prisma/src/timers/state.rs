//! El reloj de un temporizador o Pomodoro: en marcha o parado, las fases del
//! Pomodoro, modificar el tiempo con la rueda y avisar al acabar. No sabe
//! nada de teclas ni de dibujos: así se prueba solo.

use std::time::{Duration, Instant};

use super::{Kind, settings::TSettings};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Timer,
    /// Cronómetro: cuenta hacia arriba, sin final
    Stopwatch,
    Work,
    Break,
    LongBreak,
}

/// Estado de un temporizador (o Pomodoro)
#[derive(Debug, Clone)]
pub struct State {
    pub phase: Phase,
    /// Trabajo en curso del Pomodoro (1..=rondas)
    pub round: u32,
    /// Duración de la fase (cambia al modificarla)
    pub total: Duration,
    /// Lo que queda, contado hasta `since` (si está en marcha)
    left: Duration,
    since: Option<Instant>,
    /// Cuándo acabó (parpadea hasta que se pulse o pase un rato)
    pub finished: Option<Instant>,
    /// En modo modificar, hasta cuándo (se alarga al girar)
    pub editing: Option<Instant>,
    /// Último giro, para la aceleración
    last_turn: Option<Instant>,
}

/// Cuánto parpadea al acabar si nadie lo pulsa
const FLASH: Duration = Duration::from_secs(30);
/// Cuánto dura el modo modificar sin tocar la rueda
pub const EDIT_IDLE: Duration = Duration::from_secs(6);
/// Mientras se gira la rueda (y un momento después) el tiempo no parpadea,
/// para ver bien cuánto se está cambiando
pub const TURN_STEADY: Duration = Duration::from_millis(1200);

impl State {
    pub fn new(kind: Kind, s: &TSettings) -> Self {
        let phase = match kind {
            Kind::Pomodoro => Phase::Work,
            Kind::Timer if s.is_stopwatch() => Phase::Stopwatch,
            Kind::Timer => Phase::Timer,
        };
        let total = s.length(phase);
        State { phase, round: 1, total, left: total, since: None, finished: None, editing: None, last_turn: None }
    }

    pub fn running(&self) -> bool {
        self.since.is_some()
    }

    /// Lo que queda (en el cronómetro, lo que lleva)
    pub fn remaining(&self, now: Instant) -> Duration {
        if self.phase == Phase::Stopwatch {
            return self.left + self.since.map_or(Duration::ZERO, |t0| now.saturating_duration_since(t0));
        }
        match self.since {
            Some(t0) => self.left.saturating_sub(now.saturating_duration_since(t0)),
            None => self.left,
        }
    }

    pub fn is_editing(&self, now: Instant) -> bool {
        self.editing.is_some_and(|until| now < until)
    }

    /// Se está girando la rueda para cambiar el tiempo
    pub fn is_turning(&self, now: Instant) -> bool {
        self.is_editing(now) && self.last_turn.is_some_and(|t| now.saturating_duration_since(t) < TURN_STEADY)
    }

    pub fn toggle(&mut self, now: Instant) {
        self.finished = None;
        match self.since {
            Some(_) => {
                self.left = self.remaining(now);
                self.since = None;
            }
            None => {
                if self.left.is_zero() && self.phase != Phase::Stopwatch {
                    self.left = self.total;
                }
                self.since = Some(now);
            }
        }
    }

    /// Vuelve a empezar la fase (parado)
    pub fn reset(&mut self, kind: Kind, s: &TSettings) {
        *self = State::new(kind, s);
    }

    /// Pomodoro: pasa a la fase siguiente (y la empieza si `start`)
    pub fn next_phase(&mut self, s: &TSettings, now: Instant, start: bool) {
        let rounds = s.rounds.clamp(1, 12);
        self.phase = match self.phase {
            Phase::Work if self.round >= rounds => Phase::LongBreak,
            Phase::Work => Phase::Break,
            Phase::Break => {
                self.round += 1;
                Phase::Work
            }
            Phase::LongBreak => {
                self.round = 1;
                Phase::Work
            }
            p @ (Phase::Timer | Phase::Stopwatch) => p,
        };
        self.total = s.length(self.phase);
        self.left = self.total;
        self.since = start.then_some(now);
    }

    /// Cambia el tiempo girando la rueda. Los saltos crecen si se gira rápido.
    pub fn turn(&mut self, ticks: i16, now: Instant) {
        if self.phase == Phase::Stopwatch {
            return;
        }
        let gap = self.last_turn.map_or(Duration::from_secs(1), |t| now.saturating_duration_since(t));
        self.last_turn = Some(now);
        self.editing = Some(now + EDIT_IDLE);
        self.finished = None;
        let step = step_for(gap) as i64 * ticks as i64;
        let left = self.remaining(now).as_secs() as i64;
        // A pasos redondos: 1:00, 1:30… nunca por debajo de 10 s ni de 24 h
        let base = if step.abs() >= 60 { left / 60 * 60 } else { left / step.abs().max(1) * step.abs().max(1) };
        let new = (base + step).clamp(10, 24 * 3600) as u64;
        self.left = Duration::from_secs(new);
        if self.since.is_some() {
            self.since = Some(now);
        }
        if !self.running() || self.total < self.left {
            self.total = self.left;
        }
    }

    /// Avanza el reloj. Devuelve true si una fase acaba de terminar.
    pub fn tick(&mut self, kind: Kind, s: &TSettings, now: Instant) -> bool {
        if let Some(until) = self.editing
            && now >= until
        {
            self.editing = None;
        }
        if let Some(f) = self.finished
            && now.saturating_duration_since(f) > FLASH
        {
            self.finished = None;
        }
        if self.since.is_none() || self.phase == Phase::Stopwatch || !self.remaining(now).is_zero() {
            return false;
        }
        self.since = None;
        self.left = Duration::ZERO;
        self.finished = Some(now);
        if kind == Kind::Pomodoro {
            self.next_phase(s, now, s.auto_next);
        }
        true
    }
}

/// Segundos por paso según lo rápido que se gira (tiempo entre giros)
fn step_for(gap: Duration) -> u32 {
    match gap.as_millis() {
        0..=60 => 300,
        61..=150 => 60,
        151..=350 => 30,
        _ => 10,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cronometro() {
        let s = TSettings { mode: "stopwatch".into(), ..Default::default() };
        let mut x = st(Kind::Timer, &s);
        let t0 = Instant::now();
        assert_eq!(x.phase, Phase::Stopwatch);
        assert!(x.remaining(t0).is_zero());
        x.toggle(t0);
        let t1 = t0 + Duration::from_secs(90);
        assert_eq!(x.remaining(t1).as_secs(), 90);
        assert!(!x.tick(Kind::Timer, &s, t1 + Duration::from_secs(3600)));
        x.toggle(t1);
        assert_eq!(x.remaining(t1 + Duration::from_secs(50)).as_secs(), 90);
        x.toggle(t1 + Duration::from_secs(50));
        assert_eq!(x.remaining(t1 + Duration::from_secs(60)).as_secs(), 100);
        x.turn(1, t1);
        assert_eq!(x.remaining(t1 + Duration::from_secs(60)).as_secs(), 100);
        x.reset(Kind::Timer, &s);
        assert!(x.remaining(t1).is_zero() && !x.running());
    }

    #[test]
    fn fijo_mientras_se_gira() {
        let s = TSettings::default();
        let mut x = st(Kind::Timer, &s);
        let t0 = Instant::now();
        x.editing = Some(t0 + EDIT_IDLE);
        assert!(!x.is_turning(t0));
        x.turn(1, t0);
        assert!(x.is_turning(t0 + Duration::from_millis(500)));
        assert!(!x.is_turning(t0 + TURN_STEADY + Duration::from_millis(1)));
        // Al salir de modificar ya no cuenta
        x.editing = None;
        assert!(!x.is_turning(t0));
    }

    fn st(kind: Kind, s: &TSettings) -> State {
        State::new(kind, s)
    }

    #[test]
    fn temporizador() {
        let s = TSettings { minutes: 1, seconds: 30, ..Default::default() };
        let mut x = st(Kind::Timer, &s);
        let t0 = Instant::now();
        assert_eq!(x.remaining(t0), Duration::from_secs(90));
        x.toggle(t0);
        assert!(x.running());
        assert_eq!(x.remaining(t0 + Duration::from_secs(30)), Duration::from_secs(60));
        // Pausa y sigue
        x.toggle(t0 + Duration::from_secs(30));
        assert_eq!(x.remaining(t0 + Duration::from_secs(100)), Duration::from_secs(60));
        x.toggle(t0 + Duration::from_secs(100));
        assert!(!x.tick(Kind::Timer, &s, t0 + Duration::from_secs(150)));
        assert!(x.tick(Kind::Timer, &s, t0 + Duration::from_secs(161)));
        assert!(x.finished.is_some() && !x.running());
        // Volver a empezar
        x.toggle(t0 + Duration::from_secs(170));
        assert_eq!(x.remaining(t0 + Duration::from_secs(170)), Duration::from_secs(90));
    }

    #[test]
    fn pomodoro_fases() {
        let s = TSettings { work: 25, short_break: 5, long_break: 15, rounds: 2, ..Default::default() };
        let mut x = st(Kind::Pomodoro, &s);
        let t0 = Instant::now();
        assert_eq!((x.phase, x.round), (Phase::Work, 1));
        x.toggle(t0);
        assert!(x.tick(Kind::Pomodoro, &s, t0 + Duration::from_secs(25 * 60)));
        // Empieza sola el descanso
        assert_eq!(x.phase, Phase::Break);
        assert!(x.running());
        x.next_phase(&s, t0, true);
        assert_eq!((x.phase, x.round), (Phase::Work, 2));
        x.next_phase(&s, t0, true);
        assert_eq!(x.phase, Phase::LongBreak);
        x.next_phase(&s, t0, false);
        assert_eq!((x.phase, x.round), (Phase::Work, 1));
        assert_eq!(x.total, Duration::from_secs(25 * 60));
    }

    #[test]
    fn rueda_acelera() {
        let s = TSettings::default();
        let mut x = st(Kind::Timer, &s);
        let t0 = Instant::now();
        x.editing = Some(t0 + Duration::from_secs(6));
        // Despacio: de 10 en 10 s
        x.turn(1, t0);
        assert_eq!(x.remaining(t0), Duration::from_secs(5 * 60 + 10));
        // Rápido: de 5 en 5 min
        x.turn(1, t0 + Duration::from_millis(40));
        assert_eq!(x.remaining(t0), Duration::from_secs(10 * 60));
        // Nunca por debajo de 10 s
        for i in 0..50 {
            x.turn(-1, t0 + Duration::from_millis(100 + i * 30));
        }
        assert_eq!(x.remaining(t0), Duration::from_secs(10));
        assert_eq!(step_for(Duration::from_millis(200)), 30);
    }

}
