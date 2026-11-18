//! Animaciones de arranque y cierre: probarlas en el deck y subir el GIF
//! propio o la foto del final del cierre

use axum::{
    Json,
    extract::Query,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use super::{IdQuery, origin_ok};
use crate::{
    DEVICES, animation,
    family::Model,
    power, reveal,
    settings::{self, Animation},
};

use super::decks::rerender;

#[derive(Deserialize)]
pub(super) struct TestAnimBody {
    id: String,
    #[serde(default)]
    closing: bool,
    animation: Animation,
    /// Final del cierre a enseñar después (solo al probar el cierre)
    #[serde(default)]
    ending: Option<settings::EndKind>,
}

/// "Probar en el deck": reproduce la animación (sin guardar) y repinta las teclas
pub(super) async fn test_animation(headers: HeaderMap, Json(body): Json<TestAnimBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    // Si la pantalla está apagada, se enciende para ver la prueba
    let mut level = power::screen_level(&body.id).await;
    if level == 0 {
        level = 50;
        power::preview_brightness(&body.id, level).await;
    }
    if animation::busy(&body.id).await {
        return Json(json!({ "error": crate::i18n::t("error.animation_running") })).into_response();
    }
    animation::set_busy(&body.id, true).await;
    power::pause(&body.id, true).await;
    // Margen para que el deck termine lo que estuviera pintando
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let mut cascade = None;
    let result = {
        let devices = DEVICES.read().await;
        match devices.get(&body.id) {
            None => Err(crate::i18n::t("error.deck_disconnected").to_string()),
            Some(device) => match Model::from_vid_pid(device.vid, device.pid) {
                None => Err("Modelo desconocido".to_string()),
                Some(model) if !model.allows_animation(&body.animation.kind) => {
                    Err(crate::i18n::t("error.animation_unsupported").to_string())
                }
                Some(model) if body.animation.kind == settings::AnimKind::Cascade => {
                    // Cierre: se van apagando; arranque: se borran y vuelven en cascada
                    let r = if body.closing {
                        let mut r = reveal::disappear(device, &model, body.animation.duration_ms).await;
                        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
                        if let (Some(k), Ok(())) = (body.ending, &r) {
                            let ending = animation::Ending::from_kind(k, &model);
                            r = animation::preview_ending(device, &model, &body.id, ending, level).await;
                        }
                        r
                    } else {
                        animation::clear(device).await
                    };
                    cascade = Some((model, body.animation.duration_ms));
                    r.map_err(|e| e.to_string())
                }
                Some(model) => {
                    let mut r = animation::play(device, &model, &body.id, &body.animation, body.closing, level).await;
                    // Cierre: se enseña cómo se queda el deck al final
                    if let (true, Some(k), Ok(())) = (body.closing, body.ending, &r) {
                        let ending = animation::Ending::from_kind(k, &model);
                        r = animation::preview_ending(device, &model, &body.id, ending, level).await;
                    }
                    // Al terminar la prueba se deja todo como estaba
                    animation::clear(device).await.ok();
                    device.set_brightness(level).await.ok();
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    r.map_err(|e| e.to_string())
                }
            },
        }
    };
    animation::set_busy(&body.id, false).await;
    power::pause(&body.id, false).await;
    match cascade {
        // Los iconos vuelven en cascada y se espera a que termine
        Some((model, ms)) if result.is_ok() => {
            let task = tokio::spawn(reveal::appear(body.id.clone(), model, ms));
            tokio::time::sleep(std::time::Duration::from_millis(60)).await;
            rerender(&body.id).await;
            let _ = task.await;
        }
        _ => rerender(&body.id).await,
    }
    // Las pantallas de las ruedas de perfiles las pinta Prisma
    crate::profiles::redraw_device(&body.id).await;
    match result {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => Json(json!({ "error": e })).into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct GifBody {
    id: String,
    which: String,
    /// GIF como data URL
    data: String,
}

pub(super) async fn set_gif(headers: HeaderMap, Json(body): Json<GifBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some((bytes, _)) = data_url::DataUrl::process(&body.data)
        .ok()
        .and_then(|u| u.decode_to_vec().ok())
    else {
        return Json(json!({ "error": crate::i18n::t("error.file_invalid") })).into_response();
    };
    if let Err(e) = animation::validate_gif(&bytes) {
        return Json(json!({ "error": e })).into_response();
    }
    let Some(path) = animation::gif_path(&body.id, &body.which) else {
        return Json(json!({ "error": crate::i18n::t("error.no_settings_folder") })).into_response();
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, bytes) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct PhotoBody {
    id: String,
    /// Imagen como data URL
    data: String,
}

/// Foto para el final del cierre: se guarda en PNG, como mucho de 1024 px
pub(super) async fn set_photo(headers: HeaderMap, Json(body): Json<PhotoBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some((bytes, _)) = data_url::DataUrl::process(&body.data).ok().and_then(|u| u.decode_to_vec().ok()) else {
        return Json(json!({ "error": crate::i18n::t("error.file_invalid") })).into_response();
    };
    let Ok(img) = image::load_from_memory(&bytes) else {
        return Json(json!({ "error": crate::i18n::t("error.not_image") })).into_response();
    };
    let img = if img.width().max(img.height()) > 1024 { img.resize(1024, 1024, image::imageops::FilterType::Lanczos3) } else { img };
    let Some(path) = animation::photo_path(&body.id) else {
        return Json(json!({ "error": crate::i18n::t("error.no_settings_folder") })).into_response();
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match img.save_with_format(&path, image::ImageFormat::Png) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(super) async fn get_photo(Query(q): Query<IdQuery>) -> impl IntoResponse {
    match animation::photo_path(&q.id).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-store")], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct GifQuery {
    id: String,
    which: String,
}

pub(super) async fn get_gif(Query(q): Query<GifQuery>) -> impl IntoResponse {
    match animation::gif_path(&q.id, &q.which).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => ([(header::CONTENT_TYPE, "image/gif"), (header::CACHE_CONTROL, "no-store")], bytes)
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
