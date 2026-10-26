//! Vida de cada deck: conexión, registro en OpenDeck, lectura de pulsaciones,
//! envío agrupado de imágenes y keep-alive.

use std::{sync::Arc, time::Duration};

use mirajazz::{device::Device, error::MirajazzError, state::DeviceStateUpdate};
use openaction::OUTBOUND_EVENT_MANAGER;
use tokio::{sync::Notify, time::interval};
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES, FLUSH_NOTIFY, TOKENS, config,
    family::{CandidateDevice, Model},
    animation, background, led_config, leds,
    panel::{DEVICE_INFO, DeviceInfo},
    power, settings,
};

/// Inicializa un deck y atiende sus eventos hasta que se desconecta
pub async fn device_task(candidate: CandidateDevice, token: CancellationToken) {
    log::info!("Running device task for {:?}", candidate);

    let device: Result<Device, MirajazzError> = async {
        let device = connect(&candidate).await?;

        // Brillo según los ajustes de MGDC (horario de día/noche incluido)
        device
            .set_brightness(power::initial_brightness(&candidate.id).await)
            .await?;
        device.clear_all_button_images().await?;
        device.flush().await?;

        // Ajustes de LED y vibración de la familia N4 (config.toml de ambiso).
        // Si los LED se han configurado en el panel, los lleva `leds::led_task`.
        if let Model::Akp05(kind) = &candidate.model {
            let cfg = config::load();
            log::info!("Applying config: {:?}", cfg);
            let panel_leds = settings::for_device(&candidate.id).await.leds.is_some();

            if let (false, Some(led_config::LedMode::Static { colors })) = (panel_leds, cfg.leds.mode) {
                device.set_led_brightness(cfg.leds.brightness).await?;
                device.set_led_colors(&colors).await?;
            }

            if let Some(enabled) = cfg.vibration {
                if kind.supports_vibration() {
                    device.set_vibration(enabled).await?;
                } else {
                    log::warn!(
                        "Ignoring vibration config: {} does not support it",
                        kind.human_name()
                    );
                }
            }
        }

        Ok(device)
    }
    .await;

    let device = match device {
        Ok(device) => device,
        Err(err) => {
            handle_error(&candidate.id, err).await;
            log::error!(
                "Had error during device init, finishing device task: {:?}",
                candidate
            );
            return;
        }
    };

    let layout = candidate.model.layout();

    // Animación de arranque, antes de que OpenDeck pinte las teclas
    let startup = settings::for_device(&candidate.id).await.animations.startup;
    if let Err(e) = animation::play(&device, &candidate.model, &candidate.id, &startup, false).await {
        log::warn!("Animación de arranque: {e}");
    }
    if startup.kind != settings::AnimKind::None {
        animation::clear(&device).await.ok();
    }

    log::info!("Registering device {}", candidate.id);
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        outbound
            .register_device(
                candidate.id.clone(),
                candidate.model.human_name(),
                layout.rows,
                layout.cols,
                layout.encoders as u8,
                layout.device_type,
            )
            .await
            .unwrap();
    }

    DEVICE_INFO.write().await.insert(
        candidate.id.clone(),
        DeviceInfo {
            name: candidate.model.human_name(),
            model: format!("{:?}", candidate.model),
            protocol: candidate.model.protocol_version(),
            layout: candidate.model.controls().to_string(),
            screen: background::screen_size(&candidate.model),
            key_size: candidate.model.key_size(),
            vid: candidate.dev.vendor_id,
            pid: candidate.dev.product_id,
            serial: candidate.dev.serial_number.clone().unwrap_or_default(),
            connected_at: chrono::Local::now().format("%d/%m %H:%M").to_string(),
            leds: matches!(candidate.model, Model::Akp05(_)),
        },
    );

    DEVICES.write().await.insert(candidate.id.clone(), device);

    let flush_notify = Arc::new(Notify::new());
    if candidate.model.batches_flush() {
        FLUSH_NOTIFY
            .write()
            .await
            .insert(candidate.id.clone(), flush_notify.clone());
    }

    tokio::select! {
        _ = device_events_task(&candidate) => {},
        _ = async {
            if candidate.model.batches_flush() {
                device_flush_task(&candidate.id, flush_notify, token.clone()).await;
            } else {
                std::future::pending::<()>().await;
            }
        } => {},
        _ = async {
            if candidate.model.needs_keepalive() {
                keepalive_task(&candidate).await.ok();
            } else {
                std::future::pending::<()>().await;
            }
        } => {},
        _ = async {
            if matches!(candidate.model, Model::Akp05(_)) {
                leds::led_task(candidate.id.clone()).await;
            }
            std::future::pending::<()>().await;
        } => {},
        _ = token.cancelled() => {}
    };

    FLUSH_NOTIFY.write().await.remove(&candidate.id);

    log::info!("Shutting down device {:?}", candidate);

    if let Some(device) = DEVICES.read().await.get(&candidate.id) {
        // Cierre ordenado (se cierra OpenDeck): animación de cierre
        let anims = settings::for_device(&candidate.id).await.animations;
        if token.is_cancelled() {
            if let Err(e) =
                animation::play(device, &candidate.model, &candidate.id, &anims.shutdown, true).await
            {
                log::warn!("Animación de cierre: {e}");
            }
        }
        if token.is_cancelled() && anims.end_with_background {
            // Teclas en negro: se queda el fondo del deck
            animation::clear(device).await.ok();
        } else {
            device.shutdown().await.ok();
        }
    }

    log::info!("Device task finished for {:?}", candidate);
}

/// Gestiona errores: devuelve true si se puede seguir, false si es fatal
pub async fn handle_error(id: &String, err: MirajazzError) -> bool {
    log::error!("Device {} error: {}", id, err);

    // Errores leves que no implican desconexión
    if matches!(err, MirajazzError::ImageError(_) | MirajazzError::BadData) {
        return true;
    }

    log::info!("Deregistering device {}", id);
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        outbound.deregister_device(id.clone()).await.ok();
    }

    log::info!("Cancelling tasks for device {}", id);
    if let Some(token) = TOKENS.read().await.get(id) {
        token.cancel();
    }

    log::info!("Removing device {} from the list", id);
    DEVICES.write().await.remove(id);

    false
}

pub async fn connect(candidate: &CandidateDevice) -> Result<Device, MirajazzError> {
    let layout = candidate.model.layout();
    let result = Device::connect(
        &candidate.dev,
        candidate.model.protocol_version(),
        layout.keys,
        layout.encoders,
    )
    .await;

    match result {
        Ok(device) => Ok(match candidate.model.supports_both_encoder_states() {
            Some(both) => device.with_supports_both_encoder_states(both),
            None => device,
        }),
        Err(e) => {
            log::error!("Error while connecting to device: {e}");
            Err(e)
        }
    }
}

/// Envía las imágenes pendientes tras 50 ms sin cambios (familia AKP153)
async fn device_flush_task(id: &String, notify: Arc<Notify>, token: CancellationToken) {
    loop {
        tokio::select! {
            _ = notify.notified() => {}
            _ = token.cancelled() => break,
        }

        // Mientras sigan llegando imágenes, se alarga la espera
        loop {
            tokio::select! {
                _ = notify.notified() => {}
                _ = token.cancelled() => return,
                _ = tokio::time::sleep(Duration::from_millis(50)) => break,
            }
        }

        let flush_result = {
            let guard = DEVICES.read().await;
            match guard.get(id) {
                Some(device) => device.flush().await,
                None => Ok(()),
            }
        };
        if let Err(err) = flush_result {
            handle_error(id, err).await;
            break;
        }
    }
}

/// Lee las pulsaciones del deck y las pasa a OpenDeck
async fn device_events_task(candidate: &CandidateDevice) -> Result<(), MirajazzError> {
    let devices_lock = DEVICES.read().await;
    let reader = match devices_lock.get(&candidate.id) {
        Some(device) => device.get_reader(candidate.model.input_processor()),
        None => return Ok(()),
    };
    drop(devices_lock);

    log::info!("Reader is ready for {}", candidate.id);

    loop {
        let updates = match reader.read(None).await {
            Ok(updates) => updates,
            Err(e) => {
                if !handle_error(&candidate.id, e).await {
                    break;
                }
                continue;
            }
        };

        for update in updates {
            log::debug!("New update: {:#?}", update);

            let id = candidate.id.clone();

            // Durante la subida del fondo el deck manda respuestas que no son pulsaciones
            if background::muted(&id).await {
                continue;
            }

            // Cuenta como actividad; si la pantalla estaba apagada, la enciende
            // y (según ajustes) ignora esa pulsación
            let is_release = matches!(
                update,
                DeviceStateUpdate::ButtonUp(_) | DeviceStateUpdate::EncoderUp(_)
            );
            if power::on_input(&id, is_release).await {
                continue;
            }

            if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
                match update {
                    DeviceStateUpdate::ButtonDown(key) => outbound.key_down(id, key).await.unwrap(),
                    DeviceStateUpdate::ButtonUp(key) => outbound.key_up(id, key).await.unwrap(),
                    DeviceStateUpdate::EncoderDown(encoder) => {
                        outbound.encoder_down(id, encoder).await.unwrap();
                    }
                    DeviceStateUpdate::EncoderUp(encoder) => {
                        outbound.encoder_up(id, encoder).await.unwrap();
                    }
                    DeviceStateUpdate::EncoderTwist(encoder, val) => {
                        leds::on_twist(&candidate.id, encoder).await;
                        outbound
                            .encoder_change(id, encoder, val as i16)
                            .await
                            .unwrap();
                    }
                }
            }
        }
    }

    Ok(())
}

/// Keep-alive cada 10 s (familia N4)
async fn keepalive_task(candidate: &CandidateDevice) -> Result<(), MirajazzError> {
    let mut interval = interval(Duration::from_secs(10));

    loop {
        interval.tick().await;

        let devices_lock = DEVICES.read().await;
        let device = match devices_lock.get(&candidate.id) {
            Some(device) => device,
            None => return Ok(()),
        };

        if let Err(e) = device.keep_alive().await {
            drop(devices_lock);
            if !handle_error(&candidate.id, e).await {
                break;
            }
        }
    }

    Ok(())
}
