//! Panel de ajustes: servidor web local que se abre con "Ajustes del plugin".
//!
//! Las rutas de la API van por apartados del panel:
//! - `decks`: estado, ajustes, brillo, fondo, paletas, reiniciar, localidades
//! - `animations`: probar animaciones, GIF propio y foto del cierre
//! - `calibration`: imagen de prueba para calibrar los iconos
//! - `links`: enlaces entre decks
//! - `profiles`: iconos, marco, orden, jerarquía, crear y borrar perfiles

mod animations;
mod calibration;
mod decks;
mod links;
mod profiles;

use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderMap, header},
    response::Html,
    routing::{get, post},
};
use serde::Deserialize;
use tokio::sync::RwLock;

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
    /// Zonas de calibración: (nombre, [ancho, alto])
    pub cal_zones: Vec<(String, [u32; 2])>,
    /// Tipos de animación que admite
    pub anim_kinds: Vec<String>,
    pub vid: u16,
    pub pid: u16,
    pub serial: String,
}

pub static DEVICE_INFO: LazyLock<RwLock<HashMap<String, DeviceInfo>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

static PORT: OnceLock<u16> = OnceLock::new();

/// La página del panel (panel/index.html) con sus estilos y su código, que
/// van aparte por apartados y se juntan aquí en una sola página
const PANEL_HTML: &str = include_str!("../../panel/index.html");
const PANEL_CSS: &str = include_str!("../../panel/style.css");
/// En este orden: van en un solo `<script>`, así unos apartados pueden usar
/// funciones de otros que vienen después
const PANEL_JS: &[&str] = &[
    include_str!("../../panel/js/01-core.js"),
    include_str!("../../panel/js/02-tabs-decks.js"),
    include_str!("../../panel/js/03-deck-settings.js"),
    include_str!("../../panel/js/04-background.js"),
    include_str!("../../panel/js/05-palettes.js"),
    include_str!("../../panel/js/06-animations.js"),
    include_str!("../../panel/js/07-info.js"),
    include_str!("../../panel/js/08-calibration.js"),
    include_str!("../../panel/js/09-links.js"),
    include_str!("../../panel/js/10-profiles.js"),
    include_str!("../../panel/js/11-start.js"),
];

/// La página entera, sin los textos de su idioma
pub(crate) fn page_source() -> String {
    assemble(PANEL_HTML, PANEL_CSS, PANEL_JS)
}

/// Pone los estilos y el código en sus huecos (una línea con
/// `__PRISMA_CSS__` y otra con `__PRISMA_JS__`). Vale con saltos de línea de
/// Windows (CRLF) y de Linux.
fn assemble(html: &str, css: &str, js: &[&str]) -> String {
    let mut out = String::with_capacity(html.len() + css.len() + js.iter().map(|j| j.len()).sum::<usize>());
    for line in html.split_inclusive('\n') {
        match line.trim_end() {
            "__PRISMA_CSS__" => out.push_str(css),
            "__PRISMA_JS__" => js.iter().for_each(|j| out.push_str(j)),
            _ => out.push_str(line),
        }
    }
    out
}

/// La página del panel con los textos de su idioma (sección `panel` de locales/)
fn panel_page() -> String {
    let code = crate::i18n::code();
    let texts = serde_json::Value::Object(crate::i18n::section(&code, "panel")).to_string().replace("</", "<\\/");
    page_source().replace("__PRISMA_LANG__", &code).replace("__PRISMA_I18N__", &texts)
}
/// Logo de Prisma (el mismo icono del plugin) para el panel
const LOGO_PNG: &[u8] = include_bytes!("../../assets/icon@2x.png");

/// Solo se aceptan cambios desde el propio panel (no desde otras webs)
pub(crate) fn origin_ok(headers: &HeaderMap) -> bool {
    match headers.get("origin").and_then(|v| v.to_str().ok()) {
        None => true,
        Some(origin) => PORT
            .get()
            .is_some_and(|p| origin == format!("http://127.0.0.1:{p}")),
    }
}
/// Petición con solo el deck
#[derive(Deserialize)]
struct IdBody {
    id: String,
}

#[derive(Deserialize)]
struct IdQuery {
    id: String,
}

pub async fn serve() {
    let app = Router::new()
        // El panel sale en el idioma de OpenDeck, con sus textos de locales/
        .route("/", get(|| async { Html(panel_page()) }))
        .route("/logo.png", get(|| async { ([(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "max-age=86400")], LOGO_PNG) }))
        .route("/api/state", get(decks::state))
        .route("/api/sun", get(decks::sun))
        .route("/api/save", post(decks::save))
        .route("/api/brightness", post(decks::brightness))
        .route("/api/identify", post(decks::identify))
        .route("/api/background", get(decks::get_background).post(decks::set_background))
        .route("/api/palettes", post(decks::save_palettes))
        .route("/api/animation/test", post(animations::test_animation))
        .route("/api/animation/gif", get(animations::get_gif).post(animations::set_gif))
        .route("/api/animation/photo", get(animations::get_photo).post(animations::set_photo))
        .route("/api/places", get(decks::places))
        .route("/api/reset", post(decks::reset))
        .route("/api/calibration/test", post(calibration::calibration_test))
        .route("/api/calibration/stop", post(calibration::calibration_stop))
        .route("/api/links", get(links::get_links).post(links::save_links))
        .route("/api/profiles", get(profiles::get_profiles))
        .route("/api/profiles/icon", get(profiles::get_profile_icon).post(profiles::set_profile_icon))
        .route("/api/profiles/border", post(profiles::set_profile_border))
        .route("/api/profiles/create", post(profiles::create_profile))
        .route("/api/profiles/delete", post(profiles::delete_profile))
        .route("/api/profiles/rename", post(profiles::rename_profile))
        .route("/api/profiles/order", post(profiles::set_profile_order))
        .route("/api/profiles/move", post(profiles::move_profile))
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

#[cfg(test)]
mod tests {
    /// Las piezas encajan: no queda ningún hueco sin rellenar ni falta código
    #[test]
    fn pagina_completa() {
        let page = super::page_source();
        assert!(!page.contains("__PRISMA_CSS__") && !page.contains("__PRISMA_JS__"));
        assert!(page.contains("__PRISMA_LANG__") && page.contains("__PRISMA_I18N__"));
        for js in super::PANEL_JS {
            assert!(page.contains(js) && !js.contains("</script"));
        }
        assert!(page.contains(super::PANEL_CSS));
    }

    /// En Windows los ficheros llevan saltos CRLF: los huecos se rellenan igual
    #[test]
    fn saltos_de_windows() {
        let crlf = |s: &str| s.replace('\n', "\r\n");
        let html = crlf(super::PANEL_HTML);
        let css = crlf(super::PANEL_CSS);
        let js: Vec<String> = super::PANEL_JS.iter().map(|j| crlf(j)).collect();
        let js: Vec<&str> = js.iter().map(String::as_str).collect();
        let page = super::assemble(&html, &css, &js);
        assert!(!page.contains("__PRISMA_CSS__") && !page.contains("__PRISMA_JS__"));
        assert!(page.contains(&css) && js.iter().all(|j| page.contains(j)));
        assert_eq!(page, crlf(&super::page_source()));
    }

    /// La página entera, para mirarla o compararla:
    /// PRISMA_PAGE_OUT=pagina.html cargo test volcar_pagina -- --ignored
    #[test]
    #[ignore]
    fn volcar_pagina() {
        std::fs::write(std::env::var("PRISMA_PAGE_OUT").unwrap(), super::page_source()).unwrap();
    }
}
