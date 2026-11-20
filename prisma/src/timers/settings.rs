//! Ajustes de un temporizador o Pomodoro y lo que hacen click y doble click

use std::time::Duration;

use serde::Deserialize;

use super::{Kind, state::Phase};

/// Ajustes de la tecla o rueda
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct TSettings {
    /// Temporizador: "timer" (cuenta atrás) o "stopwatch" (cronómetro)
    pub mode: String,
    /// Temporizador en tecla: "digital" o "analog" (con agujas)
    pub style: String,
    /// Cronómetro: centésimas (se repinta diez veces por segundo; en el
    /// MSD-ONE puede ir lento)
    pub hundredths: bool,
    /// Temporizador: minutos y segundos
    pub minutes: u32,
    pub seconds: u32,
    /// Pomodoro: minutos de trabajo, descanso corto y largo, y cuántos
    /// trabajos hay antes del descanso largo
    pub work: u32,
    pub short_break: u32,
    pub long_break: u32,
    pub rounds: u32,
    /// Pomodoro: empezar sola la fase siguiente
    pub auto_next: bool,
    /// Sonar al acabar, y con qué sonido (ver sounds.rs)
    pub sound: bool,
    pub sound_id: String,
    /// Click y doble click: "toggle", "reset", "skip", "edit" o "none"
    /// (vacío = lo de serie para tecla o rueda)
    pub click: String,
    pub double: String,
}

impl Default for TSettings {
    fn default() -> Self {
        Self {
            mode: "timer".into(),
            style: "digital".into(),
            hundredths: false,
            minutes: 5,
            seconds: 0,
            work: 25,
            short_break: 5,
            long_break: 15,
            rounds: 4,
            auto_next: true,
            sound: true,
            sound_id: crate::sounds::BUILTIN[0].into(),
            click: String::new(),
            double: String::new(),
        }
    }
}

impl TSettings {
    pub fn is_analog(&self) -> bool {
        self.style == "analog"
    }

    pub fn is_stopwatch(&self) -> bool {
        self.mode == "stopwatch"
    }

    pub fn from_value(v: &serde_json::Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_default()
    }

    /// Lo que hacen click y doble click (con lo de serie si no se ha elegido)
    pub fn actions(&self, kind: Kind, dial: bool) -> (Action, Action) {
        let default_double = match (kind, dial) {
            (_, true) => Action::Edit,
            (Kind::Pomodoro, false) => Action::Skip,
            (Kind::Timer, false) => Action::Reset,
        };
        let pick = |s: &str, d: Action| match Action::from_id(s, dial).unwrap_or(d) {
            // El cronómetro no tiene tiempo que modificar: vuelve a cero
            Action::Edit if kind == Kind::Timer && self.is_stopwatch() => Action::Reset,
            a => a,
        };
        (pick(&self.click, Action::Toggle), pick(&self.double, default_double))
    }

    /// Duración de una fase
    pub fn length(&self, phase: Phase) -> Duration {
        let mins = |m: u32| Duration::from_secs(m.clamp(1, 600) as u64 * 60);
        match phase {
            Phase::Stopwatch => Duration::ZERO,
            Phase::Timer => Duration::from_secs((self.minutes.min(999) * 60 + self.seconds.min(59)).max(1) as u64),
            Phase::Work => mins(self.work),
            Phase::Break => mins(self.short_break),
            Phase::LongBreak => mins(self.long_break),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Toggle,
    Reset,
    Skip,
    Edit,
    None,
}

impl Action {
    /// "edit" solo vale en las ruedas
    fn from_id(id: &str, dial: bool) -> Option<Self> {
        Some(match id {
            "toggle" => Action::Toggle,
            "reset" => Action::Reset,
            "skip" => Action::Skip,
            "edit" if dial => Action::Edit,
            "none" => Action::None,
            _ => return None,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acciones_de_serie() {
        let s = TSettings::default();
        assert_eq!(s.actions(Kind::Timer, false), (Action::Toggle, Action::Reset));
        assert_eq!(s.actions(Kind::Pomodoro, false), (Action::Toggle, Action::Skip));
        assert_eq!(s.actions(Kind::Timer, true), (Action::Toggle, Action::Edit));
        // «Modificar» no vale en una tecla
        let e = TSettings { click: "edit".into(), ..Default::default() };
        assert_eq!(e.actions(Kind::Timer, false).0, Action::Toggle);
    }
}
