//! Acciones de información: hora, fecha, el tiempo y Astronomía. Valen para teclas
//! (también los laterales del MSD-ONE) y para la pantalla de las ruedas del
//! MSD-PRO. Se dibujan como SVG, igual que las de audio.
//!
//! El tiempo puede tener varias vistas (ahora, previsión, por horas…): se
//! pasa de una a otra con la rueda, al pulsar o solo cada X segundos, con un
//! deslizamiento entre una y otra.
//!
//! Si hay un aviso (lluvia en un rato, o meteorología extrema), el tiempo lo
//! enseña unos segundos y otros la vista normal; al llegar suena el sonido de
//! su tipo, y un click lo quita hasta que cambie.
//!
//! - `settings`: lo que guarda la ventana de ajustes
//! - `clock`, `date`: la hora y la fecha
//! - `weather`: las vistas del tiempo, una por fichero
//! - `astro`: Astronomía (Sol, Luna, planetas y una estrella), con vistas
//!   como el tiempo
//! - `calendar`: nombres de días y meses

mod astro;
mod calendar;
mod clock;
mod date;
mod settings;
mod weather;

use std::{
    collections::{HashMap, HashSet},
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
    Astro,
}

impl Kind {
    /// Si tiene varias vistas que se pasan con la rueda, al pulsar o solas
    fn has_views(self) -> bool {
        matches!(self, Kind::Weather | Kind::Astro)
    }

    pub fn from_uuid(uuid: &str) -> Option<Self> {
        Some(match uuid.strip_prefix(PREFIX)? {
            "clock" => Kind::Clock,
            "date" => Kind::Date,
            "weather" => Kind::Weather,
            "astro" => Kind::Astro,
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
    /// Avisos: los que se ven ahora, los quitados con un click y los que ya
    /// sonaron
    alert_shown: Vec<String>,
    dismissed: HashSet<String>,
    heard: HashSet<String>,
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

    /// La ventana del tiempo busca localidades (y la de Astronomía, estrellas)
    fn to_plugin<'a>(&'a self, e: &'a SendToPluginEvent) -> Fut<'a> {
        // Cómo va la lectura del centro de avisos elegido
        if e.payload.get("request").and_then(|r| r.as_str()) == Some("alertStatus") {
            let context = e.context.clone();
            tokio::spawn(async move {
                let Some((device, s)) = INSTANCES.lock().await.get(&context).map(|i| (i.device.clone(), i.s.clone())) else { return };
                let (lat, lon, city) = place(&device, &s).await;
                let st = if s.alert_official { crate::weather::alert_status(&s.alert_source, lat, lon, &city).await } else { None };
                canvas::reply(&context, json!({ "alertStatus": st })).await;
            });
            return done();
        }
        // La lista de organismos de avisos (y el más cercano al sitio)
        if e.payload.get("request").and_then(|r| r.as_str()) == Some("alertSources") {
            let context = e.context.clone();
            let (lat, lon) = (e.payload.get("lat").and_then(|v| v.as_f64()), e.payload.get("lon").and_then(|v| v.as_f64()));
            // `download`: la ventana pide bajarla otra vez de la OMM
            let force = e.payload.get("download").and_then(|v| v.as_bool()).unwrap_or(false);
            tokio::spawn(async move {
                let list = match crate::weather::reload_sources(force).await {
                    Ok(l) => l,
                    Err(e) => {
                        canvas::reply(&context, json!({ "alertSourcesError": format!("{} ({e})", crate::i18n::t("weather.sources_error")) })).await;
                        return;
                    }
                };
                let device = INSTANCES.lock().await.get(&context).map(|i| i.device.clone()).unwrap_or_default();
                let (lat, lon) = match (lat, lon) {
                    (Some(a), Some(b)) => (a, b),
                    _ => {
                        let sch = crate::settings::for_device(&device).await.schedule;
                        (sch.lat, sch.lon)
                    }
                };
                let auto = crate::weather::nearest_source(&list, lat, lon);
                canvas::reply(&context, json!({ "alertSources": list, "auto": auto })).await;
            });
            return done();
        }
        // Astronomía: el catálogo de estrellas, para buscar la de la vista
        if e.payload.get("request").and_then(|r| r.as_str()) == Some("stars") {
            let context = e.context.clone();
            tokio::spawn(async move {
                let list: Vec<_> = crate::stars::STARS.iter().map(|s| json!({ "n": s.name, "b": s.bayer, "c": s.con })).collect();
                canvas::reply(&context, json!({ "stars": list })).await;
            });
            return done();
        }
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
            alert_shown: vec![],
            dismissed: HashSet::new(),
            heard: HashSet::new(),
        },
    );
    refresh(context).await;
    // Al colocarla, OpenDeck puede no tener aún lista la tecla y perderse la
    // primera imagen; lo que cambia poco (la fecha) no se volvería a pintar
    // hasta el día siguiente. Se repinta una vez al momento.
    tokio::time::sleep(Duration::from_millis(800)).await;
    if let Some(i) = INSTANCES.lock().await.get_mut(context) {
        i.last.clear();
    }
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
        Some(i) if i.kind == Kind::Astro => i.s.double == "next",
        Some(i) if i.kind == Kind::Clock && !i.dial => i.s.clock_double != "none",
        _ => return,
    };
    if let Some(double) = CLICKS.press(context, has_double).await {
        act(context, double).await;
    }
}

async fn act(context: &str, double: bool) {
    // Con un aviso en pantalla, el click lo quita (hasta que cambie)
    let dismissed = INSTANCES.lock().await.get_mut(context).is_some_and(|i| {
        let shown = std::mem::take(&mut i.alert_shown);
        let any = !shown.is_empty();
        i.dismissed.extend(shown);
        i.last.clear();
        any
    });
    if dismissed {
        refresh(context).await;
        return;
    }
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
        "web" if INSTANCES.lock().await.get(context).is_some_and(|i| i.kind == Kind::Weather) => {
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
    let has_views = INSTANCES.lock().await.get(context).is_some_and(|i| i.kind.has_views());
    if has_views && ticks != 0 {
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

/// Vistas del tiempo que se enseñan ahora: las elegidas, menos «Lluvia» si
/// no va a llover en el tiempo que enseña y «Avisos» si no hay ninguno
fn views_now(s: &WSettings, dial: bool, w: Option<&crate::weather::Weather>, any_alert: bool) -> Vec<&'static str> {
    let rain = w.is_some_and(|w| weather::rain::coming(w, s.rain_span));
    let all = s.views(dial);
    let v: Vec<&'static str> = all.iter().copied().filter(|v| (*v != "rain" || rain) && (*v != "alerts" || any_alert)).collect();
    if v.is_empty() { vec!["now"] } else { v }
}

/// Cuántas vistas pasan ahora (en Astronomía, contando las páginas)
async fn view_count(kind: Kind, device: &str, s: &WSettings, dial: bool) -> usize {
    match kind {
        Kind::Astro => astro::slots(s, dial).len(),
        _ => weather_views(device, s, dial).await.len(),
    }
}

async fn weather_views(device: &str, s: &WSettings, dial: bool) -> Vec<&'static str> {
    let (lat, lon, _) = place(device, s).await;
    let any_alert = s.views.iter().any(|v| v == "alerts") && !alerts_of(device, s).await.is_empty();
    views_now(s, dial, crate::weather::get(lat, lon).await.as_ref(), any_alert)
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
            let alerts = if s.views.iter().any(|v| v == "alerts") { alerts_of(device, s).await } else { vec![] };
            let views = views_now(s, dial, w.as_ref(), !alerts.is_empty());
            let v = views[view % views.len()];
            // La luna se calcula aquí mismo (solo para las vistas que la usan)
            let sky = weather::needs_sky(v).then(|| {
                let now = chrono::Utc::now();
                (crate::astro::moon(now, lat, lon), now)
            });
            weather::body(v, dial, w.as_ref(), &city, sky.as_ref(), s.rain_span, &alerts)
        }
        Kind::Astro => {
            let (lat, lon, _) = place(device, s).await;
            let slots = astro::slots(s, dial);
            astro::body(&slots[view % slots.len()], dial, s, lat, lon, chrono::Utc::now())
        }
    }
}

/// Avisos del tiempo para una tecla o rueda: todos los que hay ahora
async fn alerts_of(device: &str, s: &WSettings) -> Vec<weather::alerts::Alert> {
    let (lat, lon, city) = place(device, s).await;
    let Some(w) = crate::weather::get(lat, lon).await else { return vec![] };
    let official = if s.alert_official { crate::weather::official(&s.alert_source, lat, lon, &city).await } else { vec![] };
    weather::alerts::all(&w, s, &official)
}

/// Cada cuántos segundos se alterna entre el aviso y la vista normal
const ALERT_EVERY: u64 = 4;

/// Si toca enseñar un aviso ahora: su dibujo y los avisos que se ven (y hace
/// sonar los nuevos)
async fn alert_now(context: &str, device: &str, dial: bool, s: &WSettings) -> Option<(String, Vec<String>)> {
    let all = alerts_of(device, s).await;
    let (visible, new): (Vec<_>, Vec<_>) = {
        let mut map = INSTANCES.lock().await;
        let i = map.get_mut(context)?;
        // Quitados que ya no están: se olvidan (si vuelven, se enseñan)
        i.dismissed.retain(|id| all.iter().any(|a| &a.id == id));
        let visible: Vec<_> = all.iter().filter(|a| !i.dismissed.contains(&a.id)).cloned().collect();
        let new: Vec<_> = visible.iter().filter(|a| i.heard.insert(a.id.clone())).cloned().collect();
        (visible, new)
    };
    for a in new {
        let snd = weather::alerts::sound_of(s, a.hazard);
        log::info!("Tiempo: aviso «{} · {}»", a.title, a.detail);
        if !snd.is_empty() {
            let snd = snd.to_string();
            tokio::task::spawn_blocking(move || crate::sounds::play(&snd));
        }
    }
    if visible.is_empty() {
        return None;
    }
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let phase = secs / ALERT_EVERY;
    if phase % 2 == 1 {
        return None;
    }
    let a = &visible[(phase / 2) as usize % visible.len()];
    Some((weather::alerts::draw(a, dial), visible.iter().map(|a| a.id.clone()).collect()))
}

/// Repinta si ha cambiado
async fn refresh(context: &str) {
    let Some((kind, device, dial, s, view)) =
        INSTANCES.lock().await.get(context).filter(|i| !i.animating).map(|i| (i.kind, i.device.clone(), i.dial, i.s.clone(), i.view))
    else {
        return;
    };
    let alert = if kind == Kind::Weather { alert_now(context, &device, dial, &s).await } else { None };
    let shown = alert.as_ref().map(|a| a.1.clone()).unwrap_or_default();
    if let Some(i) = INSTANCES.lock().await.get_mut(context) {
        i.alert_shown = shown;
    }
    let body = match alert {
        Some((svg, _)) => svg,
        None => body_of(kind, &device, dial, &s, view).await,
    };
    let svg = wrap(dial, &body);
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
    let Some((kind, device, dial, s)) = INSTANCES.lock().await.get(context).filter(|i| i.kind.has_views()).map(|i| (i.kind, i.device.clone(), i.dial, i.s.clone())) else {
        return;
    };
    let n = view_count(kind, &device, &s, dial).await;
    let Some((device, dial, s, from, to)) = ({
        let mut map = INSTANCES.lock().await;
        map.get_mut(context).filter(|i| i.kind.has_views() && !i.animating).and_then(|i| {
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
    let a = body_of(kind, &device, dial, &s, from).await;
    let b = body_of(kind, &device, dial, &s, to).await;
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
                let many = match i.kind {
                    Kind::Astro => astro::slots(&i.s, i.dial).len() > 1,
                    _ => i.s.views(i.dial).len() > 1,
                };
                let auto = i.kind.has_views()
                    && i.s.rotate_secs > 0
                    && many
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

    #[test]
    fn lluvia_solo_si_llueve() {
        let s = WSettings { views: vec!["now".into(), "rain".into()], ..Default::default() };
        let mut w = crate::weather::Weather::default();
        assert_eq!(views_now(&s, false, Some(&w), false), vec!["now"]);
        w.quarters = vec![crate::weather::Quarter { time: String::new(), mm: 0.4 }; 8];
        assert_eq!(views_now(&s, false, Some(&w), false), vec!["now", "rain"]);
        let only = WSettings { views: vec!["rain".into()], ..Default::default() };
        assert_eq!(views_now(&only, true, None, false), vec!["now"]);
    }
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
        astro::samples(&mut out);
        for (name, svg) in out {
            std::fs::write(dir.join(format!("{name}.svg")), svg).unwrap();
        }
    }
}
