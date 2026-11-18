//! Enlaces entre decks: cambiar de perfil en uno cambia también los demás

use axum::{
    Json,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde_json::json;

use super::{DEVICE_INFO, origin_ok};
use crate::DEVICES;

/// Tabla de enlaces y, para cada deck conocido, su nombre, si está conectado
/// y sus perfiles
pub(super) async fn get_links() -> impl IntoResponse {
    let l = crate::links::get();
    let connected: Vec<String> = DEVICES.read().await.keys().cloned().collect();
    // Todos: conectados, los de la tabla y cualquiera con perfiles en OpenDeck
    let on_disk = tokio::task::spawn_blocking(crate::links::all_decks).await.unwrap_or_default();
    let info = DEVICE_INFO.read().await;
    let mut ids: Vec<String> = connected.clone();
    for d in l.devices.iter().chain(on_disk.iter().map(|(id, _)| id)) {
        if !ids.contains(d) {
            ids.push(d.clone());
        }
    }
    let mut devices = vec![];
    for id in ids {
        let disk_name = on_disk.iter().find(|(d, _)| *d == id).and_then(|(_, n)| n.clone());
        let name = info
            .get(&id)
            .map(|i| i.name.clone())
            .or_else(|| l.names.get(&id).cloned())
            .or(disk_name)
            .unwrap_or_else(|| format!("Deck {id}"));
        let profiles = {
            let id = id.clone();
            tokio::task::spawn_blocking(move || crate::profiles::list_profiles(&id)).await.unwrap_or_default()
        };
        // Los perfiles en orden de árbol, con su nivel (para los desplegables)
        let tree = crate::profile_icons::tree(&id, &profiles);
        let flat: Vec<_> = crate::hierarchy::flatten(&tree)
            .into_iter()
            .map(|(p, depth)| json!({ "id": p, "name": crate::hierarchy::name_of(&tree, &p), "depth": depth }))
            .collect();
        devices.push(json!({ "id": id, "name": name, "connected": connected.contains(&id), "profiles": profiles, "tree": flat }));
    }
    Json(json!({ "links": l, "devices": devices }))
}

pub(super) async fn save_links(headers: HeaderMap, Json(body): Json<crate::links::Links>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "origen no permitido" })));
    }
    match crate::links::save(body) {
        Ok(l) => (StatusCode::OK, Json(json!({ "ok": true, "links": l }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e }))),
    }
}
