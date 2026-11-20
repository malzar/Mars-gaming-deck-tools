//! Envía a OpenDeck lo que dibujan las acciones: en las teclas con
//! `setImage` y en la pantalla de las ruedas con `setFeedback` (diseño
//! `layouts/audio-dial.json`, una sola imagen a toda la pantalla). También las
//! respuestas a las ventanas de ajustes y los ajustes que guarda el plugin.

use openaction::{OUTBOUND_EVENT_MANAGER, OutboundEventManager};
use serde_json::json;

/// Dibuja en una tecla (`dial` = false) o en la pantalla de una rueda, con el
/// gestor de eventos que ya se tiene (dentro de un manejador de eventos)
pub async fn send_with(outbound: &mut OutboundEventManager, context: &str, dial: bool, svg: &str) {
    let r = if dial {
        outbound.send_event(json!({ "event": "setFeedback", "context": context, "payload": { "full-canvas": svg } })).await
    } else {
        outbound.set_image(context.to_string(), Some(crate::svg::data_url(svg)), None).await
    };
    if let Err(e) = r {
        log::warn!("No se pudo dibujar {context}: {e}");
    }
}

/// Lo mismo desde una tarea en segundo plano. No llamarla desde un manejador
/// de eventos: el gestor ya está cogido y se quedaría esperando.
pub async fn send(context: &str, dial: bool, svg: &str) {
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        send_with(outbound, context, dial, svg).await;
    }
}

/// Contesta a la ventana de ajustes de una acción (también desde segundo plano)
pub async fn reply(context: &str, payload: serde_json::Value) {
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let _ = outbound.send_to_property_inspector(context.to_string(), payload).await;
    }
}

/// Guarda los ajustes de una acción en OpenDeck (desde segundo plano)
pub async fn set_settings(context: &str, settings: serde_json::Value) {
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut()
        && let Err(e) = outbound.set_settings(context.to_string(), settings).await
    {
        log::warn!("No se pudieron guardar los ajustes de {context}: {e}");
    }
}
