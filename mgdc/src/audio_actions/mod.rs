//! Acciones de audio de Prisma: teclas multimedia, subir/bajar/silenciar el
//! volumen (general o de un programa) y ruedas de volumen para el MSD-PRO.
//!
//! - `settings`: lo que guarda la ventana de ajustes de cada acción
//! - `state`: lo que hay en cada tecla, el último volumen y lo que suena
//! - `render`: los dibujos (SVG) de las teclas y de las ruedas
//! - `input`: pulsar, girar y pulsar la rueda
//! - `app_icons`: iconos de los programas que suenan (rueda de música)
//! - `outputs`: elegir la salida de audio desde la rueda (doble click)
//! - `tasks`: repintar lo que cambia por fuera y mover el reproductor
//! - `worker`: los hilos que hablan con Windows

mod app_icons;
mod input;
mod outputs;
mod render;
mod settings;
mod state;
mod tasks;
mod worker;

use openaction::{AppearEvent, DialPressEvent, DialRotateEvent, DidReceiveSettingsEvent, KeyEvent, OutboundEventManager, SendToPluginEvent};
use serde_json::json;

use crate::{
    actions::{ActionGroup, Fut},
    audio,
    canvas,
    clicks::Clicks,
};
use settings::{ActSettings, is_small, settings_of};
use state::{INSTANCES, Instance, refresh};

pub use tasks::{player_task, poll_task};

pub const PREFIX: &str = "com.malzar.prisma.audio.";

/// Click o doble click de las ruedas de volumen
static CLICKS: Clicks = Clicks::new();

/// Qué hace cada acción (la parte final de su UUID)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    PlayPause,
    Stop,
    Next,
    Prev,
    VolUp,
    VolDown,
    Mute,
    Dial,
    /// Rueda de música: girar cambia de pista, pulsar reproduce/pausa
    Music,
}

impl Kind {
    pub fn from_uuid(uuid: &str) -> Option<Self> {
        Some(match uuid.strip_prefix(PREFIX)? {
            "playpause" => Kind::PlayPause,
            "stop" => Kind::Stop,
            "next" => Kind::Next,
            "prev" => Kind::Prev,
            "volup" => Kind::VolUp,
            "voldown" => Kind::VolDown,
            "mute" => Kind::Mute,
            "dial" => Kind::Dial,
            "music" => Kind::Music,
            _ => return None,
        })
    }

    /// Necesita saber el volumen de su destino
    fn uses_level(self) -> bool {
        matches!(self, Kind::VolUp | Kind::VolDown | Kind::Mute | Kind::Dial)
    }

    /// Se dibuja en la pantalla de una rueda (si no, en una tecla)
    fn on_dial(self) -> bool {
        matches!(self, Kind::Dial | Kind::Music)
    }
}

/// El grupo de las acciones de audio
pub struct Audio;

impl ActionGroup for Audio {
    fn owns(&self, action: &str) -> bool {
        Kind::from_uuid(action).is_some()
    }

    fn appear<'a>(&'a self, event: &'a AppearEvent, outbound: &'a mut OutboundEventManager) -> Fut<'a> {
        Box::pin(async move {
            log::info!("Audio: aparece {} ({})", event.action, event.context);
            let Some(kind) = Kind::from_uuid(&event.action) else { return };
            let mut settings = ActSettings::from_value(&event.payload.settings);
            settings.small = is_small(&event.device).await;
            // Primero un dibujo sin datos, al momento; luego el estado real
            let first = render::render_key(kind, &settings, None, None, None);
            INSTANCES.lock().await.insert(event.context.clone(), Instance::new(kind, settings, first.clone()));
            canvas::send_with(outbound, &event.context, kind.on_dial(), &first).await;
            let context = event.context.clone();
            tokio::spawn(async move { refresh(&context).await });
        })
    }

    fn disappear<'a>(&'a self, event: &'a AppearEvent) -> Fut<'a> {
        Box::pin(async move {
            INSTANCES.lock().await.remove(&event.context);
            outputs::forget(&event.context).await;
            CLICKS.forget(&event.context).await;
        })
    }

    fn settings<'a>(&'a self, event: &'a DidReceiveSettingsEvent) -> Fut<'a> {
        Box::pin(async move {
            if let Some(i) = INSTANCES.lock().await.get_mut(&event.context) {
                let small = i.settings.small;
                i.settings = ActSettings::from_value(&event.payload.settings);
                i.settings.small = small;
                i.last.clear();
            }
            let context = event.context.clone();
            tokio::spawn(async move { refresh(&context).await });
        })
    }

    fn key_down<'a>(&'a self, event: &'a KeyEvent) -> Fut<'a> {
        Box::pin(async move {
            log::info!("Audio: tecla {}", event.action);
            let Some(kind) = Kind::from_uuid(&event.action) else { return };
            let s = settings_of(&event.context, &event.payload.settings).await;
            tokio::spawn(input::press(kind, event.context.clone(), s));
        })
    }

    fn dial_rotate<'a>(&'a self, event: &'a DialRotateEvent) -> Fut<'a> {
        Box::pin(async move {
            log::debug!("Audio: rueda {} ({} pasos)", event.action, event.payload.ticks);
            let (context, s, ticks) = (event.context.clone(), ActSettings::from_value(&event.payload.settings), event.payload.ticks);
            if Kind::from_uuid(&event.action) == Some(Kind::Music) {
                tokio::spawn(input::music_rotate(context, ticks));
                return;
            }
            tokio::spawn(async move {
                // Eligiendo salida: girar pasa por las salidas
                if !outputs::rotate(&context, ticks).await {
                    input::rotate(context, s, ticks).await;
                }
            });
        })
    }

    fn dial_down<'a>(&'a self, event: &'a DialPressEvent) -> Fut<'a> {
        Box::pin(async move {
            log::info!("Audio: rueda pulsada {}", event.action);
            let (context, s) = (event.context.clone(), ActSettings::from_value(&event.payload.settings));
            if Kind::from_uuid(&event.action) == Some(Kind::Music) {
                tokio::spawn(input::music_press(context));
                return;
            }
            tokio::spawn(async move {
                // Eligiendo salida: el click deja puesta la que se ve
                if outputs::confirm(&context).await {
                    return;
                }
                // Click silencia; doble click entra a elegir salida
                match CLICKS.press(&context, true).await {
                    Some(true) => outputs::start(context).await,
                    Some(false) => input::dial_mute(context, s).await,
                    None => {}
                }
            });
        })
    }

    /// La ventana de ajustes pregunta qué deck es, sube un GIF o pide la lista
    /// de programas con sonido
    fn to_plugin<'a>(&'a self, event: &'a SendToPluginEvent) -> Fut<'a> {
        Box::pin(async move {
            let context = event.context.clone();
            match event.payload.get("request").and_then(|r| r.as_str()).unwrap_or("") {
                // En el MSD-ONE hay menos opciones de barra
                "info" => {
                    let small = INSTANCES.lock().await.get(&context).is_some_and(|i| i.settings.small);
                    tokio::spawn(async move { canvas::reply(&context, json!({ "small": small })).await });
                }
                "apps" => {
                    tokio::spawn(async move {
                        let apps = worker::run_blocking(audio::apps).await.unwrap_or_default();
                        log::info!("Audio: programas con sonido: {apps:?}");
                        let list: Vec<_> = apps.iter().map(|a| json!({ "id": a, "name": audio::display_name(a) })).collect();
                        canvas::reply(&context, json!({ "apps": list })).await;
                    });
                }
                _ => {}
            }
        })
    }
}
