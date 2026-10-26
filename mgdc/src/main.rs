//! MGDC (Mars Gaming Deck Controller)
//!
//! Plugin de dispositivo para OpenDeck que controla en un solo proceso los decks
//! de la familia AKP153 (Mars Gaming MSD-ONE) y AKP05 / N4 (Mars Gaming MSD-PRO).
//!
//! Basado en:
//! - opendeck-akp153 de Andrey Viktorov (4ndv): https://github.com/4ndv/opendeck-akp153
//! - opendeck-akp05 de ambiso: https://github.com/ambiso/opendeck-akp05
//!
//! Licencia GPL-3.0.

use device::handle_error;
use mirajazz::device::Device;
use openaction::*;
use std::{
    collections::HashMap,
    process::exit,
    sync::{Arc, LazyLock},
};
use tokio::sync::{Mutex, Notify, RwLock};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use watcher::watcher_task;

#[cfg(not(target_os = "windows"))]
use tokio::signal::unix::{SignalKind, signal};

mod config;
mod device;
mod family;
mod led_config;
mod panel;
mod power;
mod schedule;
mod settings;
mod watcher;

pub static DEVICES: LazyLock<RwLock<HashMap<String, Device>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub static TOKENS: LazyLock<RwLock<HashMap<String, CancellationToken>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub static FLUSH_NOTIFY: LazyLock<RwLock<HashMap<String, Arc<Notify>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
pub static TRACKER: LazyLock<Mutex<TaskTracker>> = LazyLock::new(|| Mutex::new(TaskTracker::new()));

struct GlobalEventHandler {}
impl openaction::GlobalEventHandler for GlobalEventHandler {
    async fn plugin_ready(
        &self,
        _outbound: &mut openaction::OutboundEventManager,
    ) -> EventHandlerResult {
        let tracker = TRACKER.lock().await.clone();

        let token = CancellationToken::new();
        tracker.spawn(watcher_task(token.clone()));

        TOKENS
            .write()
            .await
            .insert("_watcher_task".to_string(), token);

        // Panel de ajustes y planificador de brillo/apagado
        tracker.spawn(panel::serve());
        let scheduler_token = CancellationToken::new();
        tracker.spawn(power::scheduler_task(scheduler_token.clone()));
        TOKENS
            .write()
            .await
            .insert("_scheduler_task".to_string(), scheduler_token);

        log::info!("MGDC initialized");

        Ok(())
    }

    async fn set_image(
        &self,
        event: SetImageEvent,
        _outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        log::debug!("Asked to set image: {:#?}", event);

        let id = event.device.clone();

        if let Some(device) = DEVICES.read().await.get(&event.device) {
            family::set_image(device, &id, event)
                .await
                .map_err(async |err| handle_error(&id, err).await)
                .ok();
        } else {
            log::error!("Received event for unknown device: {}", event.device);
        }

        Ok(())
    }

    async fn set_brightness(
        &self,
        event: SetBrightnessEvent,
        _outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        log::debug!("Asked to set brightness: {:#?}", event);

        // El brillo que pone OpenDeck se guarda como brillo manual del deck.
        // Si el horario de día/noche está activo, manda el horario.
        let mut s = settings::for_device(&event.device).await;
        s.brightness = event.brightness.min(100);
        settings::set_for_device(&event.device, s).await;

        Ok(())
    }

    async fn system_did_wake_up(
        &self,
        _event: SystemDidWakeUpEvent,
        outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        log::info!("The system is awake, resetting devices");

        let devices = DEVICES.write().await;

        for (id, device) in devices.iter() {
            let _ = device.reset().await;
            let _ = outbound.rerender_images(id.to_string()).await;
        }
        drop(devices);
        power::invalidate_all().await;

        Ok(())
    }

    async fn show_settings_interface(
        &self,
        _event: ShowSettingsInterfaceEvent,
        _outbound: &mut OutboundEventManager,
    ) -> EventHandlerResult {
        panel::open();
        Ok(())
    }
}

struct ActionEventHandler {}
impl openaction::ActionEventHandler for ActionEventHandler {}

async fn shutdown() {
    let tokens = TOKENS.write().await;

    for (_, token) in tokens.iter() {
        token.cancel();
    }
}

async fn connect() {
    if let Err(error) = init_plugin(GlobalEventHandler {}, ActionEventHandler {}).await {
        log::error!("Failed to initialize plugin: {}", error);
        exit(1);
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
async fn sigterm() -> Result<(), Box<dyn std::error::Error>> {
    let mut sig = signal(SignalKind::terminate())?;
    sig.recv().await;
    Ok(())
}

#[cfg(target_os = "windows")]
async fn sigterm() -> Result<(), Box<dyn std::error::Error>> {
    // En Windows solo se actúa cuando se pierde la conexión con OpenDeck
    std::future::pending::<()>().await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tokio::select! {
        _ = connect() => {},
        _ = sigterm() => {},
    }

    log::info!("Shutting down");

    shutdown().await;

    let tracker = TRACKER.lock().await.clone();

    log::info!("Waiting for tasks to finish");

    tracker.close();
    tracker.wait().await;

    log::info!("Tasks are finished, exiting now");

    Ok(())
}
