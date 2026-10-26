//! Panel de ajustes: servidor web local que se abre con "Ajustes del plugin".

use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Query},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::RwLock;

use openaction::OUTBOUND_EVENT_MANAGER;

use crate::{
    DEVICES, animation, background,
    family::Model,
    power, schedule,
    settings::{self, Animation, DeviceSettings, Palette},
};

/// Datos de cada deck conectado para mostrarlos en el panel
#[derive(Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub model: String,
    pub protocol: usize,
    pub layout: String,
    /// Tamaño de la pantalla (ancho, alto) para el fondo
    pub screen: (u32, u32),
    pub key_size: u32,
    pub vid: u16,
    pub pid: u16,
    pub serial: String,
    pub connected_at: String,
    /// Tiene LED en las ruedas
    pub leds: bool,
}

pub static DEVICE_INFO: LazyLock<RwLock<HashMap<String, DeviceInfo>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

static PORT: OnceLock<u16> = OnceLock::new();

const PANEL_HTML: &str = include_str!("../panel/index.html");

/// Solo se aceptan cambios desde el propio panel (no desde otras webs)
fn origin_ok(headers: &HeaderMap) -> bool {
    match headers.get("origin").and_then(|v| v.to_str().ok()) {
        None => true,
        Some(origin) => PORT
            .get()
            .is_some_and(|p| origin == format!("http://127.0.0.1:{p}")),
    }
}

async fn state() -> impl IntoResponse {
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
            "vid_pid": i.as_ref().map(|x| format!("{:04X}:{:04X}", x.vid, x.pid)),
            "serial": i.as_ref().map(|x| x.serial.clone()),
            "connected_at": i.as_ref().map(|x| x.connected_at.clone()),
            "has_leds": i.as_ref().is_some_and(|x| x.leds),
            "gifs": {
                "startup": animation::gif_path(&id, "startup").is_some_and(|p| p.exists()),
                "shutdown": animation::gif_path(&id, "shutdown").is_some_and(|p| p.exists()),
            },
            "settings": settings::for_device(&id).await,
        }));
    }
    devices.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let palettes = settings::SETTINGS.read().await.palettes.clone();
    Json(json!({ "version": env!("CARGO_PKG_VERSION"), "devices": devices, "palettes": palettes }))
}

#[derive(Deserialize)]
struct SunQuery {
    lat: f64,
    lon: f64,
}

async fn sun(Query(q): Query<SunQuery>) -> impl IntoResponse {
    match schedule::sun_times_local(q.lat, q.lon) {
        Some((rise, set)) => Json(json!({ "sunrise": rise, "sunset": set })),
        None => Json(json!({ "error": "polar" })),
    }
}

#[derive(Deserialize)]
struct SaveBody {
    id: String,
    settings: DeviceSettings,
}

async fn save(headers: HeaderMap, Json(body): Json<SaveBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    settings::set_for_device(&body.id, body.settings).await;
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
struct BrightnessBody {
    id: String,
    value: u8,
}

async fn brightness(headers: HeaderMap, Json(body): Json<BrightnessBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    power::preview_brightness(&body.id, body.value).await;
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
struct IdBody {
    id: String,
}

async fn identify(headers: HeaderMap, Json(body): Json<IdBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    tokio::spawn(async move { power::identify(&body.id).await });
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
struct BackgroundBody {
    id: String,
    /// Imagen como data URL (PNG o JPEG), ya al tamaño de la pantalla
    image: String,
    #[serde(default)]
    fix: background::Fix,
}

async fn set_background(headers: HeaderMap, Json(body): Json<BackgroundBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let bytes = match data_url::DataUrl::process(&body.image)
        .ok()
        .and_then(|u| u.decode_to_vec().ok())
    {
        Some((bytes, _)) => bytes,
        None => return Json(json!({ "error": "Imagen no válida" })).into_response(),
    };
    let img = match image::load_from_memory(&bytes) {
        Ok(img) => img,
        Err(e) => return Json(json!({ "error": format!("Imagen no válida: {e}") })).into_response(),
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

async fn get_background(Query(q): Query<IdBody>) -> impl IntoResponse {
    match background::saved_path(&q.id).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "no-store")], bytes)
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn save_palettes(headers: HeaderMap, Json(body): Json<Vec<Palette>>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    settings::set_palettes(body).await;
    Json(json!({ "ok": true })).into_response()
}

/// Vuelve a pedir a OpenDeck las imágenes de las teclas
async fn rerender(id: &str) {
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let _ = outbound.rerender_images(id.to_string()).await;
    }
}

#[derive(Deserialize)]
struct TestAnimBody {
    id: String,
    #[serde(default)]
    closing: bool,
    animation: Animation,
}

/// "Probar en el deck": reproduce la animación (sin guardar) y repinta las teclas
async fn test_animation(headers: HeaderMap, Json(body): Json<TestAnimBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    // Si la pantalla está apagada, se enciende para ver la prueba
    let level = power::screen_level(&body.id).await;
    if level == 0 {
        power::preview_brightness(&body.id, 50).await;
    }
    let result = {
        let devices = DEVICES.read().await;
        match devices.get(&body.id) {
            None => Err("El deck no está conectado".to_string()),
            Some(device) => match Model::from_vid_pid(device.vid, device.pid) {
                None => Err("Modelo desconocido".to_string()),
                Some(model) => {
                    let r = animation::play(device, &model, &body.id, &body.animation, body.closing).await;
                    animation::clear(device).await.ok();
                    r.map_err(|e| e.to_string())
                }
            },
        }
    };
    rerender(&body.id).await;
    match result {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => Json(json!({ "error": e })).into_response(),
    }
}

#[derive(Deserialize)]
struct GifBody {
    id: String,
    which: String,
    /// GIF como data URL
    data: String,
}

async fn set_gif(headers: HeaderMap, Json(body): Json<GifBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some((bytes, _)) = data_url::DataUrl::process(&body.data)
        .ok()
        .and_then(|u| u.decode_to_vec().ok())
    else {
        return Json(json!({ "error": "Archivo no válido" })).into_response();
    };
    if let Err(e) = animation::validate_gif(&bytes) {
        return Json(json!({ "error": e })).into_response();
    }
    let Some(path) = animation::gif_path(&body.id, &body.which) else {
        return Json(json!({ "error": "Sin carpeta de ajustes" })).into_response();
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
struct GifQuery {
    id: String,
    which: String,
}

async fn get_gif(Query(q): Query<GifQuery>) -> impl IntoResponse {
    match animation::gif_path(&q.id, &q.which).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => ([(header::CONTENT_TYPE, "image/gif"), (header::CACHE_CONTROL, "no-store")], bytes)
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// "Reiniciar deck": borra, vuelve a poner el brillo y repinta las teclas
async fn reset(headers: HeaderMap, Json(body): Json<IdBody>) -> impl IntoResponse {
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

pub async fn serve() {
    let app = Router::new()
        .route("/", get(|| async { Html(PANEL_HTML) }))
        .route("/api/state", get(state))
        .route("/api/sun", get(sun))
        .route("/api/save", post(save))
        .route("/api/brightness", post(brightness))
        .route("/api/identify", post(identify))
        .route("/api/background", get(get_background).post(set_background))
        .route("/api/palettes", post(save_palettes))
        .route("/api/animation/test", post(test_animation))
        .route("/api/animation/gif", get(get_gif).post(set_gif))
        .route("/api/reset", post(reset))
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024));

    let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => {
            log::error!("No se pudo abrir el panel: {e}");
            return;
        }
    };
    if let Ok(addr) = listener.local_addr() {
        let _ = PORT.set(addr.port());
        log::info!("Panel de ajustes en http://127.0.0.1:{}/", addr.port());
    }
    if let Err(e) = axum::serve(listener, app).await {
        log::error!("El panel se ha detenido: {e}");
    }
}

/// Abre el panel en una ventana de Edge sin barra de navegador
pub fn open() {
    let Some(port) = PORT.get() else {
        log::error!("El panel aún no está listo");
        return;
    };
    let url = format!("http://127.0.0.1:{port}/");
    log::info!("Abriendo panel: {url}");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let app = format!("--app={url}");
        let edge = std::process::Command::new("cmd")
            .args([
                "/C",
                "start",
                "",
                "msedge",
                app.as_str(),
                "--window-size=860,900",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        if !matches!(edge, Ok(s) if s.success()) {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", url.as_str()])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&url).status();
    }
}
