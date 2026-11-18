//! Temporizador y Pomodoro, en teclas y en las ruedas del MSD-PRO.
//!
//! - El tiempo de partida se pone en los ajustes de la tecla.
//! - Click y doble click hacen lo que se elija: empezar/pausar, volver a
//!   empezar, saltar de fase (Pomodoro), modificar (rueda) o nada.
//! - «Modificar» (solo en las ruedas): el tiempo parpadea y la rueda lo cambia;
//!   cuanto más rápido se gira, más grandes son los saltos. Se sale pulsando
//!   o al dejar la rueda quieta unos segundos.
//! - Al acabar parpadea, suena (si se quiere) y, en el Pomodoro, pasa a la
//!   fase siguiente.
//!
//! El estado va con la acción y no se pierde al cambiar de perfil: un
//! temporizador sigue contando aunque su tecla no esté a la vista. Cada acción
//! lleva un identificador propio en sus ajustes (`uid`), así el estado la
//! sigue si se mueve o se intercambia con otra (OpenDeck identifica las
//! teclas por su sitio, no por la acción que hay en él).
//!
//! - `settings`: ajustes y lo que hace cada pulsación
//! - `state`: el reloj (sin teclas ni dibujos)
//! - `render`: el dibujo

mod render;
mod settings;
mod state;

use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

use openaction::{AppearEvent, DialPressEvent, DialRotateEvent, DidReceiveSettingsEvent, KeyEvent, OutboundEventManager, SendToPluginEvent};
use serde_json::json;
use tokio::sync::Mutex;

use crate::{
    actions::{ActionGroup, Fut, done},
    canvas,
    clicks::Clicks,
    svg::wrap,
};
use settings::{Action, TSettings};
use state::{EDIT_IDLE, Phase, State};

pub const PREFIX: &str = "com.malzar.prisma.t.";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Timer,
    Pomodoro,
}

impl Kind {
    pub fn from_uuid(uuid: &str) -> Option<Self> {
        Some(match uuid.strip_prefix(PREFIX)? {
            "timer" => Kind::Timer,
            "pomodoro" => Kind::Pomodoro,
            _ => return None,
        })
    }
}

// ------------------------------------------------------------- instancias --

struct Instance {
    /// Identificador de la acción (ver arriba); con él se busca su estado
    uid: String,
    kind: Kind,
    dial: bool,
    s: TSettings,
    last: String,
}

static INSTANCES: LazyLock<Mutex<HashMap<String, Instance>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Estado de cada acción (por su `uid`); se queda aunque la tecla deje de verse
static STATES: LazyLock<Mutex<HashMap<String, State>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static CLICKS: Clicks = Clicks::new();

// ---------------------------------------------------------------- eventos --

/// El grupo del temporizador y el Pomodoro
pub struct Timers;

impl ActionGroup for Timers {
    fn owns(&self, action: &str) -> bool {
        Kind::from_uuid(action).is_some()
    }

    fn appear<'a>(&'a self, e: &'a AppearEvent, _outbound: &'a mut OutboundEventManager) -> Fut<'a> {
        let (c, a, k, s) = (e.context.clone(), e.action.clone(), e.payload.controller.clone(), e.payload.settings.clone());
        tokio::spawn(async move { appear(&c, &a, &k, &s).await });
        done()
    }

    fn disappear<'a>(&'a self, e: &'a AppearEvent) -> Fut<'a> {
        Box::pin(async move {
            let gone = INSTANCES.lock().await.remove(&e.context);
            CLICKS.forget(&e.context).await;
            if let Some(i) = gone {
                let device = e.device.clone();
                tokio::spawn(async move { forget_if_removed(&device, &i.uid).await });
            }
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

    fn to_plugin<'a>(&'a self, e: &'a SendToPluginEvent) -> Fut<'a> {
        let (context, payload) = (e.context.clone(), e.payload.clone());
        tokio::spawn(async move { to_plugin(&context, &payload).await });
        done()
    }
}

async fn appear(context: &str, action: &str, controller: &str, settings: &serde_json::Value) {
    let Some(kind) = Kind::from_uuid(action) else { return };
    let s = TSettings::from_value(settings);
    let saved = settings.get("uid").and_then(|u| u.as_str()).filter(|u| !u.is_empty()).map(String::from);
    let uid = saved.clone().unwrap_or_else(new_uid);
    STATES.lock().await.entry(uid.clone()).or_insert_with(|| State::new(kind, &s));
    INSTANCES.lock().await.insert(context.to_string(), Instance { uid: uid.clone(), kind, dial: controller == "Encoder", s, last: String::new() });
    match saved {
        // Sin identificador todavía (acción nueva): se guarda en sus ajustes
        None => save_uid(context, settings, &uid).await,
        // Una acción copiada lleva el mismo que la original: si las dos están
        // a la vista, la copia estrena uno (se espera un poco porque al mover
        // una acción OpenDeck puede avisar de la nueva antes de quitar la vieja)
        Some(_) => {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let copied = INSTANCES.lock().await.iter().any(|(c, i)| c != context && i.uid == uid);
            if copied {
                let fresh = new_uid();
                STATES.lock().await.insert(fresh.clone(), State::new(kind, &TSettings::from_value(settings)));
                if let Some(i) = INSTANCES.lock().await.get_mut(context) {
                    i.uid = fresh.clone();
                    i.last.clear();
                }
                save_uid(context, settings, &fresh).await;
            }
        }
    }
    refresh(context).await;
}

/// Una acción ha dejado de verse: si es porque se ha quitado (ya no está en
/// ningún perfil del deck), se para y se olvida. Si es porque se ha cambiado
/// de perfil, sigue contando. OpenDeck avisa igual en los dos casos, pero al
/// cambiar de perfil lo guarda antes y al quitarla la borra del fichero.
async fn forget_if_removed(device: &str, uid: &str) {
    tokio::time::sleep(REMOVED_CHECK).await;
    // Ha vuelto a aparecer (se ha movido) o el deck se ha desconectado
    if INSTANCES.lock().await.values().any(|i| i.uid == uid) || !crate::DEVICES.read().await.contains_key(device) {
        return;
    }
    let (d, u) = (device.to_string(), uid.to_string());
    let still_there = tokio::task::spawn_blocking(move || crate::profiles::setting_in_profiles(&d, "uid", &u)).await.unwrap_or(true);
    if !still_there && STATES.lock().await.remove(uid).is_some() {
        log::info!("Temporizador {uid} quitado de {device}: parado");
    }
}

/// Espera antes de mirar si una acción se ha quitado (OpenDeck guarda el
/// perfil justo después de avisar)
const REMOVED_CHECK: Duration = Duration::from_millis(1500);

/// Identificador nuevo para una acción
fn new_uid() -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{nanos:x}-{:x}", SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// Guarda el identificador en los ajustes de la acción, sin tocar lo demás
async fn save_uid(context: &str, settings: &serde_json::Value, uid: &str) {
    let mut v = if settings.is_object() { settings.clone() } else { json!({}) };
    v["uid"] = json!(uid);
    canvas::set_settings(context, v).await;
}

/// Identificador de la acción que hay en una tecla
async fn uid_of(context: &str) -> Option<String> {
    INSTANCES.lock().await.get(context).map(|i| i.uid.clone())
}

async fn settings_changed(context: &str, settings: &serde_json::Value) {
    let s = TSettings::from_value(settings);
    // Si la ventana de ajustes guardó sin el identificador, se vuelve a poner
    if settings.get("uid").and_then(|u| u.as_str()).is_none_or(str::is_empty)
        && let Some(uid) = uid_of(context).await
    {
        save_uid(context, settings, &uid).await;
    }
    let kind = {
        let mut map = INSTANCES.lock().await;
        let Some(i) = map.get_mut(context) else { return };
        let changed = i.s.mode != s.mode
            || i.s.minutes != s.minutes
            || i.s.seconds != s.seconds
            || i.s.work != s.work
            || i.s.short_break != s.short_break
            || i.s.long_break != s.long_break
            || i.s.rounds != s.rounds;
        i.s = s.clone();
        i.last.clear();
        changed.then_some((i.kind, i.uid.clone()))
    };
    // Nuevos tiempos: si no estaba en marcha, empieza con ellos
    if let Some((kind, uid)) = kind
        && let Some(st) = STATES.lock().await.get_mut(&uid)
        && !st.running()
    {
        st.reset(kind, &s);
    }
    refresh(context).await;
}

/// Tecla o rueda pulsada: click o doble click
async fn press(context: &str) {
    let Some(uid) = uid_of(context).await else { return };
    let now = Instant::now();
    // Pulsar mientras parpadea o se modifica: solo lo para
    {
        let mut states = STATES.lock().await;
        if let Some(st) = states.get_mut(&uid) {
            if st.is_editing(now) {
                st.editing = None;
                drop(states);
                refresh(context).await;
                return;
            }
            if st.finished.is_some() {
                st.finished = None;
                // Pomodoro que espera: la pulsación empieza la fase siguiente
                let pomodoro = INSTANCES.lock().await.get(context).is_some_and(|i| i.kind == Kind::Pomodoro);
                if pomodoro && !st.running() {
                    st.toggle(now);
                }
                drop(states);
                refresh(context).await;
                return;
            }
        }
    }
    let Some(has_double) = INSTANCES.lock().await.get(context).map(|i| i.s.actions(i.kind, i.dial).1 != Action::None) else { return };
    if let Some(double) = CLICKS.press(context, has_double).await {
        act(context, double).await;
    }
}

async fn act(context: &str, double: bool) {
    let Some((uid, kind, s, (click, dbl))) =
        INSTANCES.lock().await.get(context).map(|i| (i.uid.clone(), i.kind, i.s.clone(), i.s.actions(i.kind, i.dial)))
    else {
        return;
    };
    let now = Instant::now();
    if let Some(st) = STATES.lock().await.get_mut(&uid) {
        match if double { dbl } else { click } {
            Action::Toggle => st.toggle(now),
            Action::Reset => st.reset(kind, &s),
            Action::Skip if kind == Kind::Pomodoro => st.next_phase(&s, now, st.running()),
            Action::Skip => st.reset(kind, &s),
            Action::Edit => st.editing = Some(now + EDIT_IDLE),
            Action::None => {}
        }
    }
    refresh(context).await;
}

/// Girar la rueda: cambia el tiempo si está en modo modificar
async fn rotate(context: &str, ticks: i16) {
    let Some(uid) = uid_of(context).await else { return };
    let now = Instant::now();
    let changed = match STATES.lock().await.get_mut(&uid) {
        Some(st) if st.is_editing(now) && ticks != 0 => {
            st.turn(ticks, now);
            true
        }
        _ => false,
    };
    if changed {
        refresh(context).await;
    }
}

/// Mensajes de la ventana de ajustes: probar un sonido o guardar uno propio
async fn to_plugin(context: &str, payload: &serde_json::Value) {
    match payload.get("request").and_then(|r| r.as_str()) {
        Some("play") => {
            let id = payload.get("sound").and_then(|s| s.as_str()).unwrap_or("").to_string();
            tokio::task::spawn_blocking(move || crate::sounds::play(&id));
        }
        Some("sound") => {
            let data = payload.get("data").and_then(|d| d.as_str()).unwrap_or("").to_string();
            let r = tokio::task::spawn_blocking(move || crate::sounds::save_custom(&data)).await.unwrap_or_else(|e| Err(e.to_string()));
            let reply = match r {
                Ok(id) => json!({ "sound": id }),
                Err(e) => json!({ "soundError": e }),
            };
            canvas::reply(context, reply).await;
        }
        _ => {}
    }
}

// ------------------------------------------------------------- repintado --

/// Repinta si ha cambiado
async fn refresh(context: &str) {
    let Some((uid, kind, dial, s)) = INSTANCES.lock().await.get(context).map(|i| (i.uid.clone(), i.kind, i.dial, i.s.clone())) else { return };
    let Some(st) = STATES.lock().await.get(&uid).cloned() else { return };
    let now = Instant::now();
    // Medio segundo encendido y medio apagado
    let blink = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() / 500) % 2 == 1;
    let svg = wrap(dial, &render::body(kind, &st, &s, dial, now, blink));
    let changed = match INSTANCES.lock().await.get_mut(context) {
        Some(i) if i.last != svg => {
            i.last = svg.clone();
            true
        }
        _ => false,
    };
    if changed {
        canvas::send(context, dial, &svg).await;
    }
}

/// Avanza los temporizadores y repinta diez veces por segundo (para el
/// parpadeo). Suena al acabar una fase, aunque la tecla no esté a la vista.
pub async fn tick_task() {
    loop {
        // Cada décima: así se mueven las centésimas del cronómetro (solo se
        // reenvía lo que cambia)
        tokio::time::sleep(Duration::from_millis(100)).await;
        let now = Instant::now();
        // Ajustes de cada acción a la vista (por su uid) y sus teclas
        let (settings, contexts): (HashMap<String, (Kind, TSettings)>, Vec<String>) = {
            let map = INSTANCES.lock().await;
            (map.values().map(|i| (i.uid.clone(), (i.kind, i.s.clone()))).collect(), map.keys().cloned().collect())
        };
        let mut ring: Option<String> = None;
        {
            let mut states = STATES.lock().await;
            for (uid, st) in states.iter_mut() {
                // Sin tecla a la vista se sigue contando con los últimos ajustes conocidos
                let (kind, s) = settings.get(uid).cloned().unwrap_or_else(|| {
                    let kind = if matches!(st.phase, Phase::Timer | Phase::Stopwatch) { Kind::Timer } else { Kind::Pomodoro };
                    (kind, TSettings::default())
                });
                if st.tick(kind, &s, now) && s.sound {
                    ring = Some(s.sound_id.clone());
                }
            }
        }
        if let Some(id) = ring {
            crate::sounds::play(&id);
        }
        for context in &contexts {
            refresh(context).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMER: &str = "com.malzar.prisma.t.timer";
    const POMODORO: &str = "com.malzar.prisma.t.pomodoro";

    async fn state_at(context: &str) -> State {
        let uid = uid_of(context).await.unwrap();
        STATES.lock().await.get(&uid).cloned().unwrap()
    }

    /// Intercambiar acciones: cada una se lleva su estado (OpenDeck identifica
    /// las teclas por su sitio, no por la acción)
    #[tokio::test]
    async fn el_estado_sigue_a_la_accion() {
        let (a, b) = ("dev.P.Encoder.0.0", "dev.P.Encoder.1.0");
        appear(a, TIMER, "Encoder", &json!({ "uid": "t-1", "minutes": 5 })).await;
        appear(b, POMODORO, "Encoder", &json!({ "uid": "p-1" })).await;
        STATES.lock().await.get_mut("t-1").unwrap().toggle(Instant::now());
        assert!(state_at(a).await.running());
        // Se intercambian: OpenDeck quita cada una de su sitio y la pone en el otro
        for c in [a, b] {
            INSTANCES.lock().await.remove(c);
        }
        appear(a, POMODORO, "Encoder", &json!({ "uid": "p-1" })).await;
        appear(b, TIMER, "Encoder", &json!({ "uid": "t-1", "minutes": 5 })).await;
        assert_eq!(state_at(a).await.phase, Phase::Work);
        assert!(!state_at(a).await.running());
        assert_eq!(state_at(b).await.phase, Phase::Timer);
        assert!(state_at(b).await.running());
    }

    /// Una copia de una acción (mismo uid en sus ajustes) estrena estado
    #[tokio::test]
    async fn una_copia_estrena_estado() {
        let (a, b) = ("dev.Q.Keypad.0.0", "dev.Q.Keypad.1.0");
        appear(a, TIMER, "Keypad", &json!({ "uid": "t-2" })).await;
        STATES.lock().await.get_mut("t-2").unwrap().toggle(Instant::now());
        appear(b, TIMER, "Keypad", &json!({ "uid": "t-2" })).await;
        assert_eq!(uid_of(a).await.as_deref(), Some("t-2"));
        assert_ne!(uid_of(b).await.as_deref(), Some("t-2"));
        assert!(state_at(a).await.running() && !state_at(b).await.running());
    }

    /// Una acción nueva (sin uid) recibe uno
    #[tokio::test]
    async fn accion_nueva() {
        let c = "dev.R.Keypad.0.0";
        appear(c, TIMER, "Keypad", &json!({})).await;
        let uid = uid_of(c).await.unwrap();
        assert!(!uid.is_empty() && STATES.lock().await.contains_key(&uid));
    }
}
