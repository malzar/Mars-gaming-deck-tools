//! Lo que hacen los gestos de la pantalla táctil (ver `gestures`): el toque
//! que espera por si llega un deslizamiento, y el deslizamiento en sí.

use std::time::Duration;

use openaction::{OUTBOUND_EVENT_MANAGER, OutboundEventManager};

use crate::{
    audio::MediaKey,
    gestures::{self, Swipe, WINDOW_MAX, WINDOW_MIN},
};

/// Volumen general que cambia cada deslizamiento
const VOLUME_STEP: i32 = 2;

async fn window(device: &str) -> Duration {
    let ms = crate::settings::for_device(device).await.swipe_window_ms.clamp(WINDOW_MIN, WINDOW_MAX);
    Duration::from_millis(ms as u64)
}

/// Manda a OpenDeck un toque que estaba esperando
async fn send_tap(device: String, encoder: u8) {
    // Que la rueda de perfiles lo siga viendo como toque
    gestures::mark_touch(encoder as usize);
    if let Some(o) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let _ = o.encoder_down(device.clone(), encoder).await;
        let _ = o.encoder_up(device, encoder).await;
    }
}

/// Pulsan una rueda. Si es un toque en una rueda con deslizamiento, se queda
/// esperando (devuelve true: no hay que mandarlo ahora)
pub async fn hold_tap(device: &str, encoder: u8) -> bool {
    let Some((seq, prev)) = gestures::hold(device, encoder) else { return false };
    if let Some(p) = prev {
        tokio::spawn(send_tap(p.device, p.encoder));
    }
    let wait = window(device).await;
    tokio::spawn(async move {
        tokio::time::sleep(wait).await;
        if let Some(h) = gestures::expire(seq) {
            send_tap(h.device, h.encoder).await;
        }
    });
    true
}

/// Deslizan por la pantalla (`dir`: +1 derecha, −1 izquierda)
pub async fn swipe(outbound: &mut OutboundEventManager, device: &str, dir: i8) {
    let Some((encoder, cfg)) = gestures::take_swipe(device, window(device).await) else {
        // Sin toque antes: como hasta ahora, para la rueda de perfiles
        let id = device.to_string();
        tokio::spawn(async move { crate::profiles::swipe(&id, -(dir as i16)).await });
        return;
    };
    let ticks = cfg.ticks(dir);
    log::info!("Pantalla: deslizar {ticks:+} en la rueda {encoder} ({:?})", cfg.what);
    match cfg.what {
        Swipe::Rotate => {
            let _ = outbound.encoder_change(device.to_string(), encoder, ticks).await;
        }
        Swipe::Track => {
            let key = if ticks > 0 { MediaKey::Next } else { MediaKey::Prev };
            tokio::task::spawn_blocking(move || crate::media::send(key));
        }
        Swipe::Volume => {
            tokio::task::spawn_blocking(move || crate::audio::change_volume("general", ticks as i32 * VOLUME_STEP));
        }
    }
}
