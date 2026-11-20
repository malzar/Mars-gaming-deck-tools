//! Perfiles: iconos, color del marco, orden, jerarquía, crear y borrar

use axum::{
    Json,
    extract::Query,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use super::origin_ok;
use crate::DEVICES;

#[derive(Deserialize)]
pub(super) struct ProfilesQuery {
    id: String,
}

/// Perfiles de un deck con su icono y el color del marco del activo
pub(super) async fn get_profiles(Query(q): Query<ProfilesQuery>) -> impl IntoResponse {
    let id = q.id.clone();
    let (profiles, active) = tokio::task::spawn_blocking(move || {
        (crate::profiles::list_profiles(&id), crate::profiles::active_profile(&id))
    })
    .await
    .unwrap_or_default();
    // En orden de árbol: cada perfil seguido de sus subperfiles
    let tree = crate::profile_icons::tree(&q.id, &profiles);
    let look = crate::profile_icons::look(&q.id);
    let list: Vec<_> = crate::hierarchy::flatten(&tree)
        .into_iter()
        .map(|(p, depth)| {
            json!({
                "name": p,
                "nombre": crate::hierarchy::name_of(&tree, &p),
                "padre": crate::hierarchy::parent_of(&tree, &p),
                "depth": depth,
                "kids": crate::hierarchy::children(&tree, Some(&p)).len(),
                "active": active.as_deref() == Some(p.as_str()),
                "icon": look.icons.get(&p),
            })
        })
        .collect();
    Json(json!({ "profiles": list, "border": crate::profile_icons::border(&q.id) }))
}

#[derive(Deserialize)]
pub(super) struct IconQuery {
    id: String,
    profile: String,
}

/// El icono de un perfil tal cual (el GIF se mueve también en el panel)
pub(super) async fn get_profile_icon(Query(q): Query<IconQuery>) -> impl IntoResponse {
    let Some(path) = crate::profile_icons::icon_file(&q.id, &q.profile) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let kind = if path.extension().is_some_and(|e| e == "gif") { "image/gif" } else { "image/png" };
    match std::fs::read(path) {
        Ok(bytes) => ([(header::CONTENT_TYPE, kind), (header::CACHE_CONTROL, "no-store")], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
pub(super) struct IconBody {
    id: String,
    profile: String,
    /// "data:…;base64,…" para ponerlo, null para quitarlo
    image: Option<String>,
}

pub(super) async fn set_profile_icon(headers: HeaderMap, Json(b): Json<IconBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "origen no permitido" })));
    }
    let (id, profile) = (b.id.clone(), b.profile.clone());
    let r = tokio::task::spawn_blocking(move || crate::profile_icons::set_icon(&id, &profile, b.image.as_deref()))
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match r {
        Ok(()) => {
            crate::profiles::redraw_device(&b.id).await;
            (StatusCode::OK, Json(json!({ "ok": true, "icon": crate::profile_icons::look(&b.id).icons.get(&b.profile) })))
        }
        Err(e) => (StatusCode::OK, Json(json!({ "error": e }))),
    }
}

#[derive(Deserialize)]
pub(super) struct BorderBody {
    id: String,
    border: String,
}

pub(super) async fn set_profile_border(headers: HeaderMap, Json(b): Json<BorderBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "origen no permitido" })));
    }
    match crate::profile_icons::set_border(&b.id, &b.border) {
        Ok(()) => {
            crate::profiles::redraw_device(&b.id).await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Err(e) => (StatusCode::OK, Json(json!({ "error": e }))),
    }
}

#[derive(Deserialize)]
pub(super) struct OrderBody {
    id: String,
    /// Padre de los que se ordenan (None = raíz)
    padre: Option<String>,
    order: Vec<String>,
}

/// Orden de los perfiles en la rueda
pub(super) async fn set_profile_order(headers: HeaderMap, Json(b): Json<OrderBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "origen no permitido" })));
    }
    let id = b.id.clone();
    let existing = tokio::task::spawn_blocking(move || crate::profiles::list_profiles(&id)).await.unwrap_or_default();
    let mut tree = crate::profile_icons::tree(&b.id, &existing);
    crate::hierarchy::reorder(&mut tree, b.padre.as_deref(), &b.order);
    match crate::profile_icons::save_tree(&b.id, tree) {
        Ok(()) => {
            crate::profiles::redraw_device(&b.id).await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Err(e) => (StatusCode::OK, Json(json!({ "error": e }))),
    }
}

#[derive(Deserialize)]
pub(super) struct MoveBody {
    id: String,
    /// Perfil que se mueve (con sus subperfiles)
    profile: String,
    /// Dónde va: dentro de este perfil (None = raíz)
    padre: Option<String>,
    /// Orden de los de ese grupo, ya con el que se mueve
    order: Vec<String>,
}

/// Mueve un perfil: a otro sitio de su grupo, a otro grupo o dentro de otro
/// perfil (así un perfil pasa a subperfil y al revés)
pub(super) async fn move_profile(headers: HeaderMap, Json(b): Json<MoveBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "origen no permitido" })));
    }
    let id = b.id.clone();
    let existing = tokio::task::spawn_blocking(move || crate::profiles::list_profiles(&id)).await.unwrap_or_default();
    let mut tree = crate::profile_icons::tree(&b.id, &existing);
    let padre = b.padre.filter(|p| existing.contains(p));
    if let Err(e) = crate::hierarchy::set_parent(&mut tree, &b.profile, padre.as_deref()) {
        return (StatusCode::OK, Json(json!({ "error": e })));
    }
    crate::hierarchy::reorder(&mut tree, padre.as_deref(), &b.order);
    match crate::profile_icons::save_tree(&b.id, tree) {
        Ok(()) => {
            crate::profiles::redraw_device(&b.id).await;
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Err(e) => (StatusCode::OK, Json(json!({ "error": e }))),
    }
}

#[derive(Deserialize)]
pub(super) struct DeleteBody {
    id: String,
    name: String,
    /// También sus subperfiles (si no, suben un nivel)
    #[serde(default)]
    all: bool,
}

/// Borra un perfil (y sus subperfiles, si se pide). Como al crearlo, el deck se desconecta un momento de
/// OpenDeck para que relea los perfiles.
pub(super) async fn delete_profile(headers: HeaderMap, Json(b): Json<DeleteBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "origen no permitido" })));
    }
    if !DEVICES.read().await.contains_key(&b.id) {
        return (StatusCode::OK, Json(json!({ "error": crate::i18n::t("error.deck_disconnected") })));
    }
    // Antes de desconectar nada: que se pueda borrar
    let (id, name, all) = (b.id.clone(), b.name.clone(), b.all);
    let ok = tokio::task::spawn_blocking(move || crate::profiles::check_delete(&id, &name, all)).await.unwrap_or_else(|e| Err(e.to_string()));
    if let Err(e) = ok {
        return (StatusCode::OK, Json(json!({ "error": e })));
    }
    let (id, name, all) = (b.id.clone(), b.name.clone(), b.all);
    let r = crate::device::reload_profiles(&b.id, move || crate::profiles::delete_profile(&id, &name, all))
        .await
        .unwrap_or_else(|| Err(crate::i18n::t("error.profile_delete").into()));
    crate::profiles::redraw_device(&b.id).await;
    match r {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true }))),
        Err(e) => (StatusCode::OK, Json(json!({ "error": e }))),
    }
}

#[derive(Deserialize)]
pub(super) struct RenameBody {
    id: String,
    name: String,
    new_name: String,
}

/// Cambia el nombre de un perfil (también en OpenDeck). Como al crearlo, el
/// deck se desconecta un momento de OpenDeck para que relea los perfiles.
pub(super) async fn rename_profile(headers: HeaderMap, Json(b): Json<RenameBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "origen no permitido" })));
    }
    if !DEVICES.read().await.contains_key(&b.id) {
        return (StatusCode::OK, Json(json!({ "error": crate::i18n::t("error.deck_disconnected") })));
    }
    // Antes de desconectar nada: que se pueda y que el nombre valga
    let (id, old, new) = (b.id.clone(), b.name.clone(), b.new_name.clone());
    let checked = tokio::task::spawn_blocking(move || crate::profiles::check_rename(&id, &old, &new)).await.unwrap_or_else(|e| Err(e.to_string()));
    let new = match checked {
        Ok(n) if n == b.name => return (StatusCode::OK, Json(json!({ "ok": true, "name": n }))),
        Ok(n) => n,
        Err(e) => return (StatusCode::OK, Json(json!({ "error": e }))),
    };
    let (id, old, n) = (b.id.clone(), b.name.clone(), new.clone());
    let r = crate::device::reload_profiles(&b.id, move || crate::profiles::rename_profile(&id, &old, &n))
        .await
        .unwrap_or_else(|| Err(crate::i18n::t("error.profile_rename").into()));
    if r.is_ok() {
        crate::profiles::rename_in_wheel(&b.id, &b.name, &new).await;
    }
    crate::profiles::redraw_device(&b.id).await;
    match r {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true, "name": new }))),
        Err(e) => (StatusCode::OK, Json(json!({ "error": e }))),
    }
}

#[derive(Deserialize)]
pub(super) struct CreateBody {
    id: String,
    name: String,
    /// Perfil del que copiar las teclas (None = vacío)
    copy_of: Option<String>,
    /// Perfil dentro del que va (None = raíz): un subperfil
    padre: Option<String>,
}

/// Crea un perfil. OpenDeck lo ve al momento: se desconecta el deck de
/// OpenDeck, se escribe el perfil (y su «Switch Profile» en las teclas
/// virtuales de todos) y se vuelve a conectar.
pub(super) async fn create_profile(headers: HeaderMap, Json(b): Json<CreateBody>) -> impl IntoResponse {
    if !origin_ok(&headers) {
        return (StatusCode::FORBIDDEN, Json(json!({ "error": "origen no permitido" })));
    }
    let model = {
        let devices = DEVICES.read().await;
        devices.get(&b.id).and_then(|d| crate::family::Model::from_vid_pid(d.vid, d.pid))
    };
    let Some(model) = model else {
        return (StatusCode::OK, Json(json!({ "error": crate::i18n::t("error.deck_disconnected") })));
    };
    // Antes de desconectar nada: que el nombre valga
    let (id, name) = (b.id.clone(), b.name.clone());
    let checked = tokio::task::spawn_blocking(move || crate::profiles::check_name(&id, &name)).await.unwrap_or_else(|e| Err(e.to_string()));
    let name = match checked {
        Ok(n) => n,
        Err(e) => return (StatusCode::OK, Json(json!({ "error": e }))),
    };
    let copy_of = b.copy_of.filter(|c| !c.is_empty());
    let (id, n) = (b.id.clone(), name.clone());
    let r = crate::device::reload_profiles(&b.id, move || crate::profiles::create_profile(&id, &model, &n, copy_of.as_deref()))
        .await
        .unwrap_or_else(|| Err(crate::i18n::t("error.profile_create").into()));
    // Su sitio en la jerarquía
    if r.is_ok() {
        let id = b.id.clone();
        let existing = tokio::task::spawn_blocking(move || crate::profiles::list_profiles(&id)).await.unwrap_or_default();
        let mut tree = crate::profile_icons::tree(&b.id, &existing);
        let padre = b.padre.filter(|p| existing.contains(p));
        if crate::hierarchy::set_parent(&mut tree, &name, padre.as_deref()).is_ok() {
            let _ = crate::profile_icons::save_tree(&b.id, tree);
        }
        crate::profiles::redraw_device(&b.id).await;
    }
    match r {
        Ok(()) => (StatusCode::OK, Json(json!({ "ok": true, "name": name }))),
        Err(e) => (StatusCode::OK, Json(json!({ "error": e }))),
    }
}
