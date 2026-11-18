//! Los decks: estado para el panel, ajustes, brillo, fondo, paletas,
//! identificar, reiniciar y buscar localidades para el horario

use axum::{
    Json,
    extract::Query,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use super::{IdBody, origin_ok};
use crate::{
    DEVICES, animation, background,
    power, schedule,
    settings::{self, DeviceSettings, Palette},
};

use super::DEVICE_INFO;
use openaction::OUTBOUND_EVENT_MANAGER;

pub(super) async fn state() -> impl IntoResponse {
    let connected: Vec<String> = DEVICES.read().await.keys().cloned().collect();
    let info = DEVICE_INFO.read().await;
    let mut devices = vec![];
    for id in connected {
        let i = info.get(&id).cloned();
        devices.push(json!({
            "id": id,
            "name": i.as_ref().map(|x| x.name.clone()).unwrap_or_else(|| id.clone()),
            "model": i.as_ref().map(|x| x.model.clone()).unwrap_or_default(),
            "protocol": i.as_ref().map(|x| x.protocol).unwrap_or(0),
            "layout": i.as_ref().map(|x| x.layout.clone()).unwrap_or_default(),
            "screen": i.as_ref().map(|x| [x.screen.0, x.screen.1]),
            "has_background": background::saved_path(&id).is_some_and(|p| p.exists()),
            "key_size": i.as_ref().map(|x| x.key_size),
            "cal_zones": i.as_ref().map(|x| x.cal_zones.clone()),
            "anim_kinds": i.as_ref().map(|x| x.anim_kinds.clone()),
            "vid_pid": i.as_ref().map(|x| format!("{:04X}:{:04X}", x.vid, x.pid)),
            "serial": i.as_ref().map(|x| x.serial.clone()),
            "gifs": {
                "startup": animation::gif_path(&id, "startup").is_some_and(|p| p.exists()),
                "shutdown": animation::gif_path(&id, "shutdown").is_some_and(|p| p.exists()),
            },
            "photo": animation::photo_path(&id).and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok()).map(|t| t.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())),
            "settings": settings::for_device(&id).await,
        }));
    }
    devices.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let palettes = settings::SETTINGS.read().await.palettes.clone();
    Json(json!({ "version": env!("CARGO_PKG_VERSION"), "devices": devices, "palettes": palettes }))
}

#[derive(Deserialize)]
pub(super) struct SunQuery {
    lat: f64,
    lon: f64,
}

pub(super) async fn sun(Query(q): Query<SunQuery>) -> impl IntoResponse {
    match schedule::sun_times_local(q.lat, q.lon) {
        Some((rise, set)) => Json(json!({ "sunrise": rise, "sunset": set })),
        None => Json(json!({ "error": "polar" })),
    }
}

#[derive(Deserialize)]
pub(super) struct SaveBody {
    id: String,
    settings: DeviceSettings,
}

pub(super) async fn save(headers: HeaderMap, Json(body): Json<SaveBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    settings::set_for_device(&body.id, body.settings).await;
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
pub(super) struct BrightnessBody {
    id: String,
    value: u8,
}

pub(super) async fn brightness(headers: HeaderMap, Json(body): Json<BrightnessBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    power::preview_brightness(&body.id, body.value).await;
    Json(json!({ "ok": true })).into_response()
}

pub(super) async fn identify(headers: HeaderMap, Json(body): Json<IdBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    tokio::spawn(async move { power::identify(&body.id).await });
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
pub(super) struct BackgroundBody {
    id: String,
    /// Imagen como data URL (PNG o JPEG), ya al tamaño de la pantalla
    image: String,
    #[serde(default)]
    fix: background::Fix,
}

pub(super) async fn set_background(headers: HeaderMap, Json(body): Json<BackgroundBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let bytes = match data_url::DataUrl::process(&body.image)
        .ok()
        .and_then(|u| u.decode_to_vec().ok())
    {
        Some((bytes, _)) => bytes,
        None => return Json(json!({ "error": crate::i18n::t("error.image_invalid") })).into_response(),
    };
    let img = match image::load_from_memory(&bytes) {
        Ok(img) => img,
        Err(e) => return Json(json!({ "error": crate::i18n::tf("error.image_invalid_detail", &[&e]) })).into_response(),
    };
    match background::upload(&body.id, &img, body.fix).await {
        Ok(()) => {
            background::save_copy(&body.id, &img);
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => {
            log::error!("{e}");
            Json(json!({ "error": e })).into_response()
        }
    }
}

pub(super) async fn get_background(Query(q): Query<IdBody>) -> impl IntoResponse {
    match background::saved_path(&q.id).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-store")], bytes)
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub(super) async fn save_palettes(headers: HeaderMap, Json(body): Json<Vec<Palette>>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    settings::set_palettes(body).await;
    Json(json!({ "ok": true })).into_response()
}

/// Vuelve a pedir a OpenDeck las imágenes de las teclas
pub(crate) async fn rerender(id: &str) {
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let _ = outbound.rerender_images(id.to_string()).await;
    }
}

#[derive(Deserialize)]
pub(super) struct PlacesQuery {
    q: String,
}

/// Busca localidades para el horario de día y noche
pub(super) async fn places(Query(q): Query<PlacesQuery>) -> impl IntoResponse {
    let lang = crate::i18n::code();
    match tokio::task::spawn_blocking(move || crate::weather::search_places(&q.q, &lang)).await {
        Ok(Ok(list)) => Json(json!({ "places": list })),
        Ok(Err(e)) => Json(json!({ "error": e })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// "Reiniciar deck": borra, vuelve a poner el brillo y repinta las teclas
pub(super) async fn reset(headers: HeaderMap, Json(body): Json<IdBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let ok = match DEVICES.read().await.get(&body.id) {
        Some(device) => device.reset().await.is_ok(),
        None => false,
    };
    power::invalidate_all().await;
    rerender(&body.id).await;
    Json(json!({ "ok": ok })).into_response()
}
