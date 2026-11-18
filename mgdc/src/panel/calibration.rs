//! Calibración de los iconos: imagen de prueba en el deck con la
//! calibración que se está ajustando (sin guardar)

use std::{collections::HashMap, sync::LazyLock};

use axum::{
    Json,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use super::{IdBody, origin_ok};
use crate::{
    DEVICES, animation,
    family::Model,
    settings::Calibration,
};

use tokio::sync::RwLock;
use super::decks::rerender;

#[derive(Deserialize)]
pub(super) struct CalibrationBody {
    id: String,
    calibration: Calibration,
    /// "keys", "side" o vacío (todas)
    #[serde(default)]
    zone: String,
}

/// Última vez que se usó la imagen de prueba en cada deck
static CAL_LAST: LazyLock<RwLock<HashMap<String, std::time::Instant>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Por si se cierra el panel sin salir de la prueba: a los 5 minutos sin
/// tocar nada, vuelven los iconos de OpenDeck
fn cal_watchdog(id: String) {
    tokio::spawn(async move {
        let idle = std::time::Duration::from_secs(300);
        tokio::time::sleep(idle).await;
        let expired = CAL_LAST.read().await.get(&id).is_some_and(|t| t.elapsed() >= idle);
        if expired && animation::busy(&id).await {
            log::info!("Calibración de {id} abandonada: se vuelven a pintar los iconos");
            animation::set_busy(&id, false).await;
            rerender(&id).await;
        }
    });
}

/// Una calibración a la vez, para no saturar el MSD-ONE
static CAL_LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Pinta la imagen de prueba con la calibración indicada (sin guardar)
pub(super) async fn calibration_test(headers: HeaderMap, Json(body): Json<CalibrationBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let _guard = CAL_LOCK.lock().await;
    CAL_LAST.write().await.insert(body.id.clone(), std::time::Instant::now());
    cal_watchdog(body.id.clone());
    if !animation::busy(&body.id).await {
        animation::set_busy(&body.id, true).await;
        // Margen para que el deck termine lo que estuviera pintando
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    let result = async {
        let devices = DEVICES.read().await;
        let device = devices.get(&body.id).ok_or(crate::i18n::t("error.deck_disconnected"))?;
        let model = Model::from_vid_pid(device.vid, device.pid).ok_or("Modelo desconocido")?;
        let sent = model
            .draw_calibration(device, &body.zone, &body.calibration)
            .await
            .map_err(|e| e.to_string())? as u64;
        device.flush().await.map_err(|e| e.to_string())?;
        // Respiro entre tandas (el MSD-ONE lo necesita más)
        let (base, per_key) = model.anim_pacing();
        tokio::time::sleep(std::time::Duration::from_millis(base + per_key * sent)).await;
        Ok::<(), String>(())
    }
    .await;

    match result {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => Json(json!({ "error": e })).into_response(),
    }
}

/// Sale del modo de prueba y vuelve a pintar los iconos de OpenDeck
pub(super) async fn calibration_stop(headers: HeaderMap, Json(body): Json<IdBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let _guard = CAL_LOCK.lock().await;
    animation::set_busy(&body.id, false).await;
    rerender(&body.id).await;
    Json(json!({ "ok": true })).into_response()
}
