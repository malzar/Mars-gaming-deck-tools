//! La fecha: día de la semana, día y mes (y el año en la rueda)

use chrono::{Datelike, Local};

use super::calendar::{day_name, month_name};
use crate::svg::{ACCENT, MUTED, text};

pub fn body(dial: bool, now: &chrono::DateTime<Local>) -> String {
    let day = day_name(now.weekday().num_days_from_monday() as usize);
    let month = month_name(now.month0() as usize);
    if dial {
        return format!(
            "{}{}{}",
            text(100.0, 26.0, 21.0, ACCENT, true, &day.to_uppercase()),
            text(100.0, 68.0, 40.0, "#fff", true, &format!("{} {}", now.day(), month.chars().take(3).collect::<String>())),
            text(100.0, 94.0, 19.0, MUTED, true, &now.year().to_string())
        );
    }
    format!(
        r##"<rect width="144" height="38" fill="{ACCENT}"/>{}{}{}"##,
        text(72.0, 27.0, 20.0, "#001a2c", true, &day.to_uppercase()),
        text(72.0, 102.0, 62.0, "#fff", true, &now.day().to_string()),
        text(72.0, 128.0, 17.0, MUTED, false, month)
    )
}
