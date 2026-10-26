//! Detecta los decks conectados y los que se enchufan o desenchufan.

use futures_lite::StreamExt;
use mirajazz::{
    device::{DeviceWatcher, list_devices},
    error::MirajazzError,
    types::{DeviceLifecycleEvent, HidDeviceInfo},
};
use openaction::OUTBOUND_EVENT_MANAGER;
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES, TOKENS, TRACKER,
    device::device_task,
    family::{CandidateDevice, Model, QUERIES},
};

fn device_info_to_candidate(dev: HidDeviceInfo) -> Option<CandidateDevice> {
    let model = Model::from_vid_pid(dev.vendor_id, dev.product_id)?;
    let id = model.device_id(&dev)?;

    Some(CandidateDevice { id, dev, model })
}

/// Dispositivos ya conectados que coinciden con algún VID/PID conocido
async fn get_candidates() -> Result<Vec<CandidateDevice>, MirajazzError> {
    log::info!("Looking for candidate devices");

    let mut candidates: Vec<CandidateDevice> = Vec::new();

    for dev in list_devices(&QUERIES).await? {
        if let Some(candidate) = device_info_to_candidate(dev.clone()) {
            candidates.push(candidate);
        }
    }

    Ok(candidates)
}

async fn spawn_device(candidate: CandidateDevice) {
    let tracker = TRACKER.lock().await.clone();
    let token = CancellationToken::new();

    TOKENS
        .write()
        .await
        .insert(candidate.id.clone(), token.clone());

    tracker.spawn(device_task(candidate, token));
}

pub async fn watcher_task(token: CancellationToken) -> Result<(), MirajazzError> {
    for candidate in get_candidates().await? {
        log::info!("New candidate {:#?}", candidate);
        spawn_device(candidate).await;
    }

    let mut watcher = DeviceWatcher::new();
    let mut watcher_stream = watcher.watch(&QUERIES).await?;

    log::info!("Watcher is ready");

    loop {
        let ev = tokio::select! {
            v = watcher_stream.next() => v,
            _ = token.cancelled() => None
        };

        let Some(ev) = ev else {
            log::info!("Watcher is shutting down");
            break Ok(());
        };

        log::info!("New device event: {:?}", ev);

        match ev {
            DeviceLifecycleEvent::Connected(info) => {
                if let Some(candidate) = device_info_to_candidate(info) {
                    // No añadir dos veces el mismo dispositivo
                    if DEVICES.read().await.contains_key(&candidate.id) {
                        continue;
                    }
                    spawn_device(candidate).await;
                }
            }
            DeviceLifecycleEvent::Disconnected(info) => {
                let Some(id) = device_info_to_candidate(info).map(|c| c.id) else {
                    log::warn!("Disconnected device without a known id");
                    continue;
                };

                if let Some(token) = TOKENS.write().await.remove(&id) {
                    log::info!("Sending cancel request for {}", id);
                    token.cancel();
                }

                DEVICES.write().await.remove(&id);

                if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
                    outbound.deregister_device(id.clone()).await.ok();
                }

                log::info!("Disconnected device {}", id);
            }
        }
    }
}
