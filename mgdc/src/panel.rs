//! Panel de ajustes: servidor web local que se abre con "Ajustes del plugin".

use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use axum::{
    Json, Router,
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::RwLock;

use crate::{
    DEVICES, power, schedule,
    settings::{self, DeviceSettings},
};

/// Datos de cada deck conectado para mostrarlos en el panel
#[derive(Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub model: String,
    pub protocol: usize,
    pub layout: String,
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
            "settings": settings::for_device(&id).await,
        }));
    }
    devices.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Json(json!({ "version": env!("CARGO_PKG_VERSION"), "devices": devices }))
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

pub async fn serve() {
    let app = Router::new()
        .route("/", get(|| async { Html(PANEL_HTML) }))
        .route("/api/state", get(state))
        .route("/api/sun", get(sun))
        .route("/api/save", post(save))
        .route("/api/brightness", post(brightness))
        .route("/api/identify", post(identify));

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
