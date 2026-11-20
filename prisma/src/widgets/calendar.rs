//! Nombres de los días y de los meses en el idioma de OpenDeck

use chrono::{Datelike, NaiveDate};

use crate::i18n::t;

/// Claves de los días (de lunes a domingo) y de los meses
const DAYS: [&str; 7] = ["day.mon", "day.tue", "day.wed", "day.thu", "day.fri", "day.sat", "day.sun"];
const MONTHS: [&str; 12] = [
    "month.jan", "month.feb", "month.mar", "month.apr", "month.may", "month.jun", "month.jul", "month.aug", "month.sep", "month.oct", "month.nov",
    "month.dec",
];

/// Día de la semana: 0 = lunes
pub fn day_name(i: usize) -> &'static str {
    t(DAYS[i % 7])
}

/// Mes: 0 = enero
pub fn month_name(i: usize) -> &'static str {
    t(MONTHS[i % 12])
}

/// Las tres primeras letras, en mayúsculas ("MAR")
pub fn short_day(d: &str) -> String {
    d.chars().take(3).collect::<String>().to_uppercase()
}

/// Nombre corto del día de una fecha "2026-11-17"
pub fn weekday_of(date: &str) -> String {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").map(|d| short_day(day_name(d.weekday().num_days_from_monday() as usize))).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn dia_de_una_fecha() {
        assert_eq!(super::weekday_of("2026-11-17"), "MAR");
        assert_eq!(super::weekday_of("no"), "");
    }
}
