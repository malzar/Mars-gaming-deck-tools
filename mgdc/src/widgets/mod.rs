//! Acciones de información: hora, fecha y el tiempo. Valen para teclas
//! (también los laterales del MSD-ONE) y para la pantalla de las ruedas del
//! MSD-PRO. Se dibujan como SVG, igual que las de audio.
//!
//! El tiempo puede tener varias vistas (ahora, previsión, por horas…): se
//! pasa de una a otra con la rueda, al pulsar o solo cada X segundos, con un
//! deslizamiento entre una y otra.
//!
//! - `settings`: lo que guarda la ventana de ajustes
//! - `clock`, `date`: la hora y la fecha
//! - `weather`: las vistas del tiempo, una por fichero
//! - `calendar`: nombres de días y meses

mod calendar;
mod clock;
mod date;
mod settings;
mod weather;

use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

use chrono::Local;
use openaction::{AppearEvent, DialPressEvent, DialRotateEvent, DidReceiveSettingsEvent, KeyEvent, OutboundEventManager, SendToPluginEvent};
use serde_json::json;
use tokio::sync::Mutex;

use crate::{
    actions::{ActionGroup, Fut, done},
    canvas,
    clicks::Clicks,
    svg::{DIAL_W, KEY, wrap},
};
use settings::WSettings;

pub const PREFIX: &str = "com.malzar.prisma.w.";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Clock,
    Date,
    Weather,
}

impl Kind {
    pub fn from_uuid(uuid: &str) -> Option<Self> {
        Some(match uuid.strip_prefix(PREFIX)? {
            "clock" => Kind::Clock,
            "date" => Kind::Date,
            "weather" => Kind::Weather,
            _ => return None,
        })
    }
}

struct Instance {
    kind: Kind,
    device: String,
    dial: bool,
    s: WSettings,
    /// Vista del tiempo que se ve
    view: usize,
    /// Cuándo se cambió de vista por última vez (para el cambio automático)
    switched: Instant,
    /// Último dibujo enviado
    last: String,
    /// Durante el deslizamiento el refresco no la toca
    animating: bool,
    /// Los ajustes tal cual (para guardar cambios sin perder nada)
    raw: serde_json::Value,
}

static INSTANCES: LazyLock<Mutex<HashMap<String, Instance>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static CLICKS: Clicks = Clicks::new();

// ---------------------------------------------------------------- eventos --

/// El grupo de las acciones de hora, fecha y tiempo
pub struct Widgets;

impl ActionGroup for Widgets {
    fn owns(&self, action: &str) -> bool {
        Kind::from_uuid(action).is_some()
    }

    fn appear<'a>(&'a self, e: &'a AppearEvent, _outbound: &'a mut OutboundEventManager) -> Fut<'a> {
        let (c, d, a, k, s) = (e.context.clone(), e.device.clone(), e.action.clone(), e.payload.controller.clone(), e.payload.settings.clone());
        tokio::spawn(async move { appear(&c, &d, &a, &k, &s).await });
        done()
    }

    fn disappear<'a>(&'a self, e: &'a AppearEvent) -> Fut<'a> {
        Box::pin(async move {
            INSTANCES.lock().await.remove(&e.context);
            CLICKS.forget(&e.context).await;
        })
    }

    fn settings<'a>(&'a self, e: &'a DidReceiveSettingsEvent) -> Fut<'a> {
        let (context, settings) = (e.context.clone(), e.payload.settings.clone());
        tokio::spawn(async move { settings_changed(&context, &settings).await });
        done()
    }

    fn key_down<'a>(&'a self, e: &'a KeyEvent) -> Fut<'a> {
        let context = e.context.clone();
        tokio::spawn(async move { press(&context).await });
        done()
    }

    fn dial_rotate<'a>(&'a self, e: &'a DialRotateEvent) -> Fut<'a> {
        let (context, ticks) = (e.context.clone(), e.payload.ticks);
        tokio::spawn(async move { rotate(&context, ticks).await });
        done()
    }

    fn dial_down<'a>(&'a self, e: &'a DialPressEvent) -> Fut<'a> {
        let context = e.context.clone();
        tokio::spawn(async move { press(&context).await });
        done()
    }

    /// La ventana del tiempo busca localidades
    fn to_plugin<'a>(&'a self, e: &'a SendToPluginEvent) -> Fut<'a> {
        if e.payload.get("request").and_then(|r| r.as_str()) == Some("places") {
            let q = e.payload.get("q").and_then(|q| q.as_str()).unwrap_or("").to_string();
            let context = e.context.clone();
            tokio::spawn(async move {
                let lang = crate::i18n::code();
                let q2 = q.clone();
                let reply = match tokio::task::spawn_blocking(move || crate::weather::search_places(&q2, &lang)).await {
                    Ok(Ok(list)) => json!({ "places": list, "q": q }),
                    Ok(Err(e)) => json!({ "placesError": e, "q": q }),
                    Err(e) => json!({ "placesError": e.to_string(), "q": q }),
                };
                canvas::reply(&context, reply).await;
            });
        }
        done()
    }
}

async fn appear(context: &str, device: &str, action: &str, controller: &str, settings: &serde_json::Value) {
    let Some(kind) = Kind::from_uuid(action) else { return };
    let dial = controller == "Encoder";
    INSTANCES.lock().await.insert(
        context.to_string(),
        Instance {
            kind,
            device: device.to_string(),
            dial,
            s: WSettings::from_value(settings),
            view: 0,
            switched: Instant::now(),
            last: String::new(),
            animating: false,
            raw: settings.clone(),
        },
    );
    refresh(context).await;
}

async fn settings_changed(context: &str, settings: &serde_json::Value) {
    if let Some(i) = INSTANCES.lock().await.get_mut(context) {
        i.s = WSettings::from_value(settings);
        i.raw = settings.clone();
        i.view = 0;
        i.switched = Instant::now();
        i.last.clear();
    }
    refresh(context).await;
}

/// Tecla pulsada o rueda pulsada: click o doble click (solo el tiempo)
async fn press(context: &str) {
    let has_double = match INSTANCES.lock().await.get(context) {
        Some(i) if i.kind == Kind::Weather => i.s.has_double(),
        Some(i) if i.kind == Kind::Clock && !i.dial => i.s.clock_double != "none",
        _ => return,
    };
    if let Some(double) = CLICKS.press(context, has_double).await {
        act(context, double).await;
    }
}

async fn act(context: &str, double: bool) {
    let Some((what, device, s)) = INSTANCES.lock().await.get(context).map(|i| {
        let what = match (i.kind, double) {
            (Kind::Clock, false) => i.s.clock_click.clone(),
            (Kind::Clock, true) => i.s.clock_double.clone(),
            (_, false) => i.s.click.clone(),
            (_, true) => i.s.double.clone(),
        };
        (what, i.device.clone(), i.s.clone())
    }) else {
        return;
    };
    match what.as_str() {
        "next" => switch(context, 1).await,
        "style" => toggle_style(context).await,
        "web" => {
            let (lat, lon, _) = place(&device, &s).await;
            crate::desktop::open_url(&crate::weather::web_url(lat, lon));
        }
        _ => {}
    }
}

/// Hora: pasa de agujas a digital o al revés, y lo guarda en sus ajustes
async fn toggle_style(context: &str) {
    let Some(raw) = INSTANCES.lock().await.get_mut(context).map(|i| {
        i.s.style = if i.s.style == "analog" { "digital".into() } else { "analog".into() };
        if !i.raw.is_object() {
            i.raw = json!({});
        }
        i.raw["style"] = json!(i.s.style);
        i.last.clear();
        i.raw.clone()
    }) else {
        return;
    };
    refresh(context).await;
    canvas::set_settings(context, raw).await;
}

/// Girar la rueda: cambia de vista
async fn rotate(context: &str, ticks: i16) {
    let is_weather = INSTANCES.lock().await.get(context).is_some_and(|i| i.kind == Kind::Weather);
    if is_weather && ticks != 0 {
        switch(context, ticks.signum() as i32).await;
    }
}

// ----------------------------------------------------------------- dibujo --

/// Sitio del tiempo: el de la acción o el del horario del panel
async fn place(device: &str, s: &WSettings) -> (f64, f64, String) {
    match (s.lat, s.lon) {
        (Some(lat), Some(lon)) => (lat, lon, s.city.clone()),
        _ => {
            let sch = crate::settings::for_device(device).await.schedule;
            (sch.lat, sch.lon, sch.city)
        }
    }
}

/// Dibujo de una vista (cuerpo sin el `<svg>`)
async fn body_of(kind: Kind, device: &str, dial: bool, s: &WSettings, view: usize) -> String {
    let now = Local::now();
    match kind {
        Kind::Clock => clock::body(s, dial, &now),
        Kind::Date => date::body(dial, &now),
        Kind::Weather => {
            let (lat, lon, city) = place(device, s).await;
            let w = crate::weather::get(lat, lon).await;
            let views = s.views(dial);
            let v = views[view % views.len()];
            // La luna se calcula aquí mismo (solo para las vistas que la usan)
            let sky = weather::needs_sky(v).then(|| {
                let now = chrono::Utc::now();
                (crate::astro::moon(now, lat, lon), now)
            });
            weather::body(v, dial, w.as_ref(), &city, sky.as_ref())
        }
    }
}

/// Repinta si ha cambiado
async fn refresh(context: &str) {
    let Some((kind, device, dial, s, view)) =
        INSTANCES.lock().await.get(context).filter(|i| !i.animating).map(|i| (i.kind, i.device.clone(), i.dial, i.s.clone(), i.view))
    else {
        return;
    };
    let svg = wrap(dial, &body_of(kind, &device, dial, &s, view).await);
    let changed = match INSTANCES.lock().await.get_mut(context) {
        Some(i) if i.last != svg && !i.animating => {
            i.last = svg.clone();
            true
        }
        _ => false,
    };
    if changed {
        canvas::send(context, dial, &svg).await;
    }
}

/// Pasa a la vista siguiente (dir > 0) o anterior deslizando la una sobre la otra
async fn switch(context: &str, dir: i32) {
    let Some((device, dial, s, from, to)) = ({
        let mut map = INSTANCES.lock().await;
        map.get_mut(context).filter(|i| i.kind == Kind::Weather && !i.animating).and_then(|i| {
            let n = i.s.views(i.dial).len();
            if n < 2 {
                return None;
            }
            let from = i.view % n;
            let to = (from as i32 + dir).rem_euclid(n as i32) as usize;
            i.view = to;
            i.switched = Instant::now();
            i.animating = true;
            Some((i.device.clone(), i.dial, i.s.clone(), from, to))
        })
    }) else {
        return;
    };
    let a = body_of(Kind::Weather, &device, dial, &s, from).await;
    let b = body_of(Kind::Weather, &device, dial, &s, to).await;
    let w = if dial { DIAL_W } else { KEY };
    // Teclas: menos fotogramas (cada imagen pasa por OpenDeck)
    let frames = if dial { 6 } else { 4 };
    for f in 1..=frames {
        let t = f as f32 / frames as f32;
        let e = 1.0 - (1.0 - t).powi(3);
        let off = e * w * dir.signum() as f32;
        let svg = wrap(
            dial,
            &format!(r##"<g transform="translate({:.1} 0)">{a}</g><g transform="translate({:.1} 0)">{b}</g>"##, -off, w * dir.signum() as f32 - off),
        );
        canvas::send(context, dial, &svg).await;
        tokio::time::sleep(Duration::from_millis(if dial { 40 } else { 60 })).await;
    }
    if let Some(i) = INSTANCES.lock().await.get_mut(context) {
        i.animating = false;
        i.last.clear();
    }
    refresh(context).await;
}

/// Repinta cada segundo lo que cambie (la hora, los datos del tiempo) y hace
/// los cambios de vista automáticos
pub async fn tick_task() {
    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let items: Vec<(String, bool)> = INSTANCES
            .lock()
            .await
            .iter()
            .map(|(c, i)| {
                let auto = i.kind == Kind::Weather
                    && i.s.rotate_secs > 0
                    && i.s.views(i.dial).len() > 1
                    && i.switched.elapsed() >= Duration::from_secs(i.s.rotate_secs.max(2) as u64);
                (c.clone(), auto)
            })
            .collect();
        for (context, auto) in items {
            if auto {
                let c = context.clone();
                tokio::spawn(async move { switch(&c, 1).await });
            } else {
                refresh(&context).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Vistas de muestra en SVG: cargo test --release -- --ignored muestras_widgets
    #[test]
    #[ignore]
    fn muestras_widgets() {
        samples("target/widgets");
        crate::i18n::force(Some("en"));
        samples("target/widgets/en");
        crate::i18n::force(None);
    }

    fn samples(dir: &str) {
        let now = Local.with_ymd_and_hms(2026, 11, 16, 18, 56, 42).unwrap();
        let dir = std::path::Path::new(dir);
        std::fs::create_dir_all(dir).unwrap();
        let mut s = WSettings::default();
        let mut out: Vec<(String, String)> = vec![];
        out.push(("hora".into(), wrap(false, &clock::body(&s, false, &now))));
        s.style = "analog".into();
        out.push(("agujas".into(), wrap(false, &clock::body(&s, false, &now))));
        s.style = "digital".into();
        s.part = "hours".into();
        out.push(("horas".into(), wrap(false, &clock::body(&s, false, &now))));
        s.part = "minutes".into();
        out.push(("minutos".into(), wrap(false, &clock::body(&s, false, &now))));
        out.push(("hora-rueda".into(), wrap(true, &clock::body(&s, true, &now))));
        out.push(("fecha".into(), wrap(false, &date::body(false, &now))));
        out.push(("fecha-rueda".into(), wrap(true, &date::body(true, &now))));
        weather::samples(&mut out);
        for (name, svg) in out {
            std::fs::write(dir.join(format!("{name}.svg")), svg).unwrap();
        }
    }
}
