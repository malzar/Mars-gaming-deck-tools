//! Las vistas del tiempo. Cada una está en su fichero y se apunta en
//! [`VIEWS`] con dónde vale (tecla, rueda o las dos). Para añadir una vista
//! basta con escribirla y apuntarla aquí.
//!
//! Tamaños pensados para leerse en el deck: la tecla de 144 se ve a 112 px en
//! el MSD-PRO y a 85 en el MSD-ONE, así que nada baja de 16 en las teclas ni
//! de 14 en las ruedas; mejor pocos datos y grandes.

mod air;
mod days;
mod moon;
mod now;
mod pollen;
mod sun;
mod uv;
mod wind;

use chrono::Local;

use crate::{
    astro::Moon,
    i18n::t,
    svg::{ACCENT, MUTED, clip, text},
    weather::Weather,
};

/// La luna en este momento (solo se calcula para las vistas que la usan)
pub type Sky = (Moon, chrono::DateTime<chrono::Utc>);

/// Lo que necesita una vista para dibujarse
pub struct Ctx<'a> {
    pub w: &'a Weather,
    pub city: &'a str,
    pub sky: Option<&'a Sky>,
}

/// Una vista del tiempo
pub struct View {
    /// Nombre en los ajustes ("now", "wind"…)
    pub id: &'static str,
    pub on_key: bool,
    pub on_dial: bool,
    /// Usa la luna
    pub sky: bool,
    /// Dibujo (cuerpo sin el `<svg>`); el `bool` es si es una rueda
    pub draw: fn(&Ctx, bool) -> String,
}

/// Todas las vistas, en el orden de la ventana de ajustes
pub static VIEWS: &[View] = &[
    View { id: "now", on_key: true, on_dial: true, sky: false, draw: now::draw },
    View { id: "forecast", on_key: true, on_dial: true, sky: false, draw: days::forecast },
    View { id: "today", on_key: true, on_dial: false, sky: false, draw: days::today },
    View { id: "tomorrow", on_key: true, on_dial: false, sky: false, draw: days::tomorrow },
    View { id: "after", on_key: true, on_dial: false, sky: false, draw: days::after },
    View { id: "hours", on_key: false, on_dial: true, sky: false, draw: days::hours },
    View { id: "wind", on_key: true, on_dial: true, sky: false, draw: wind::draw },
    View { id: "uv", on_key: true, on_dial: true, sky: false, draw: uv::draw },
    View { id: "air", on_key: true, on_dial: true, sky: false, draw: air::draw },
    View { id: "pollen", on_key: true, on_dial: true, sky: false, draw: pollen::draw },
    View { id: "sun", on_key: true, on_dial: true, sky: true, draw: sun::draw },
    View { id: "moon", on_key: true, on_dial: true, sky: true, draw: moon::draw },
];

/// La vista `id` si vale para tecla (o rueda)
pub fn view(id: &str, dial: bool) -> Option<&'static View> {
    VIEWS.iter().find(|v| v.id == id && if dial { v.on_dial } else { v.on_key })
}

/// Si la vista necesita la luna
pub fn needs_sky(id: &str) -> bool {
    VIEWS.iter().any(|v| v.id == id && v.sky)
}

/// Dibujo de una vista (cuerpo sin el `<svg>`). Sin datos todavía: «cargando».
/// Una vista que no vale ahí se ve como «ahora».
pub fn body(id: &str, dial: bool, w: Option<&Weather>, city: &str, sky: Option<&Sky>) -> String {
    let Some(w) = w else {
        let (cx, cy) = if dial { (100.0, 50.0) } else { (72.0, 68.0) };
        return format!("{}{}", text(cx, cy, 20.0, MUTED, false, t("weather.loading")), text(cx, cy + 26.0, 17.0, MUTED, false, &clip(city, 14)));
    };
    let draw = view(id, dial).map_or(now::draw as fn(&Ctx, bool) -> String, |v| v.draw);
    draw(&Ctx { w, city, sky }, dial)
}

// ------------------------------------------------- piezas de las vistas --

fn temp(t: f32) -> String {
    format!("{}°", t.round() as i32)
}

/// Gota de lluvia pequeña (en la rueda)
fn drop(x: f32, y: f32) -> String {
    format!(r##"<path d="M{x} {y}c-3 5-5 7-5 9a5 5 0 0 0 10 0c0-2-2-4-5-9z" fill="{ACCENT}"/>"##, y = y - 1.0)
}

/// Hora local "HH:MM" (o "--:--")
fn hm(t: Option<chrono::DateTime<chrono::Utc>>) -> String {
    t.map(|t| t.with_timezone(&Local).format("%H:%M").to_string()).unwrap_or_else(|| "--:--".into())
}

#[cfg(test)]
pub use tests::samples;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::wrap;
    use chrono::TimeZone;

    #[test]
    fn cada_vista_vale_en_algun_sitio() {
        for v in VIEWS {
            assert!(v.on_key || v.on_dial, "{}", v.id);
            assert_eq!(VIEWS.iter().filter(|x| x.id == v.id).count(), 1, "{} repetida", v.id);
        }
        assert!(needs_sky("sun") && needs_sky("moon") && !needs_sky("now"));
        assert!(view("hours", false).is_none() && view("hours", true).is_some());
    }

    /// Vistas de muestra en SVG (las saca `muestras_widgets`)
    pub fn samples(out: &mut Vec<(String, String)>) {
        let mut w = crate::weather::parse(include_str!("../../../tests/open-meteo.json")).unwrap();
        w.air = Some(crate::weather::parse_air(r#"{"current":{"european_aqi":27,"pm10":12.5,"pm2_5":7.1,"grass_pollen":34.0,"olive_pollen":12.0,"birch_pollen":0.0,"alder_pollen":0.0,"mugwort_pollen":0.0,"ragweed_pollen":6.0}}"#).unwrap());
        let at = |h: u32| {
            let t = chrono::Utc.with_ymd_and_hms(2026, 11, 16, h, 0, 0).unwrap();
            (crate::astro::moon(t, 40.4168, -3.7038), t)
        };
        let day_sky = at(14);
        for v in ["now", "forecast", "today", "tomorrow", "after", "wind", "uv", "air", "pollen", "sun", "moon"] {
            out.push((format!("tiempo-{v}"), wrap(false, &body(v, false, Some(&w), "Madrid", Some(&day_sky)))));
        }
        // De noche (la luna fuera y sin salir)
        let mut night = w.clone();
        night.time = "2026-11-16T21:00".into();
        for (name, h) in [("noche-luna", 20u32), ("noche-sin-luna", 2)] {
            let sky = at(h);
            out.push((format!("tiempo-sun-{name}"), wrap(false, &body("sun", false, Some(&night), "Madrid", Some(&sky)))));
            out.push((format!("tiempo-rueda-sun-{name}"), wrap(true, &body("sun", true, Some(&night), "Madrid", Some(&sky)))));
        }
        for v in ["now", "forecast", "hours", "wind", "uv", "air", "pollen", "sun", "moon"] {
            out.push((format!("tiempo-rueda-{v}"), wrap(true, &body(v, true, Some(&w), "Madrid", Some(&day_sky)))));
        }
        out.push(("cargando".into(), wrap(false, &body("now", false, None, "Madrid", None))));
    }
}

