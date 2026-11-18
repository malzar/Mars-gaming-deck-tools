//! Las teclas virtuales: unas teclas de más que Prisma le dice a OpenDeck que
//! tiene el deck (solo existen en el editor, en una fila debajo de las
//! ruedas). En cada una va un «Switch Profile»; para cambiar de perfil, Prisma
//! «pulsa» la que salta a él.

use std::{collections::HashMap, sync::LazyLock};

use openaction::{OUTBOUND_EVENT_MANAGER, SetImageEvent};
use tokio::sync::Mutex;

use super::store::{active_profile, read_profile};

/// La acción «Switch Profile» de OpenDeck
pub(super) const SWITCH_UUID: &str = "com.amansprojects.starterpack.switchprofile";

/// Imagen que OpenDeck manda para cada tecla virtual (None = hueco vacío), por deck
static SLOT_IMAGES: LazyLock<Mutex<HashMap<String, Vec<Option<String>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Teclas virtuales de un modelo: (primera posición, cuántas). Van detrás de
/// las de verdad.
/// Sin límite práctico: la fila crece una tecla por perfil hasta aquí
pub const MAX_VIRTUAL: u8 = 100;

pub fn virtual_keys(model: &crate::family::Model) -> Option<(u8, u8)> {
    let l = model.layout();
    (l.virtual_rows > 0).then(|| (l.rows * l.cols, MAX_VIRTUAL))
}

/// Teclas virtuales que tiene ahora cada deck en OpenDeck: tantas como haga
/// falta (una por perfil), como mucho las de `virtual_keys`
pub(super) static COUNTS: LazyLock<std::sync::RwLock<HashMap<String, u8>>> = LazyLock::new(|| std::sync::RwLock::new(HashMap::new()));
/// Cuántas teclas virtuales registrar para este deck
pub fn virtual_count(device: &str) -> u8 {
    COUNTS.read().unwrap().get(device).copied().unwrap_or(0)
}

/// (primera tecla virtual, cuántas hay ahora) de un deck conectado
pub(super) async fn first_virtual(device: &str) -> Option<(u8, u8)> {
    let devices = crate::DEVICES.read().await;
    let d = devices.get(device)?;
    let (first, _) = virtual_keys(&crate::family::Model::from_vid_pid(d.vid, d.pid)?)?;
    Some((first, virtual_count(device)))
}

/// Si OpenDeck manda la imagen de una tecla virtual, se guarda aquí (no hay
/// pantalla donde pintarla) y se repintan las ruedas de ese deck.
/// Devuelve true si la imagen era de una tecla virtual.
pub async fn intercept(evt: &SetImageEvent) -> bool {
    let Some(pos) = evt.position else { return false };
    if evt.controller.as_deref() == Some("Encoder") {
        return false;
    }
    let Some((first, n)) = first_virtual(&evt.device).await else { return false };
    if pos < first || pos >= first + n {
        return false;
    }
    let slot = (pos - first) as usize;
    SLOT_IMAGES.lock().await.entry(evt.device.clone()).or_insert_with(|| vec![None; n as usize])[slot] =
        evt.image.clone().filter(|i| !i.is_empty());
    let device = evt.device.clone();
    tokio::spawn(async move { super::draw::redraw_device(&device).await });
    true
}

/// Qué hay en cada hueco de la fila virtual de un perfil
#[derive(Debug, Clone, PartialEq)]
pub enum Slot {
    Empty,
    /// Un "Switch Profile" que salta a ese perfil
    Switch(String),
    /// Otra acción
    Other,
}

pub(super) fn row_of(profile: &serde_json::Value, first: usize, n: usize) -> Vec<Slot> {
    let keys = profile.get("keys").and_then(|k| k.as_array());
    (0..n)
        .map(|i| match keys.and_then(|k| k.get(first + i)) {
            None | Some(serde_json::Value::Null) => Slot::Empty,
            Some(inst) if inst.pointer("/action/uuid").and_then(|u| u.as_str()) == Some(SWITCH_UUID) => Slot::Switch(
                inst.pointer("/settings/profile").and_then(|p| p.as_str()).unwrap_or("Default").to_string(),
            ),
            Some(_) => Slot::Other,
        })
        .collect()
}

/// Olvida las imágenes de las teclas virtuales (OpenDeck las vuelve a mandar)
pub async fn forget_images(device: &str) {
    SLOT_IMAGES.lock().await.remove(device);
}

/// Cambia un deck a un perfil "pulsando" la tecla virtual que salta a él en el
/// perfil activo de ese deck. Devuelve si la encontró.
pub async fn switch_device(device: &str, profile: &str) -> bool {
    let Some((first, n)) = first_virtual(device).await else { return false };
    let row = {
        let d = device.to_string();
        tokio::task::spawn_blocking(move || {
            let active = active_profile(&d).unwrap_or_else(|| "Default".into());
            read_profile(&d, &active).map(|v| row_of(&v, first as usize, n as usize))
        })
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
    };
    let Some(slot) = row.iter().position(|s| *s == Slot::Switch(profile.to_string())) else {
        log::warn!("Enlaces: en {device} falta el «Switch Profile» a «{profile}» en las teclas virtuales (se añade al reiniciar OpenDeck)");
        return false;
    };
    let position = first + slot as u8;
    if let Some(outbound) = OUTBOUND_EVENT_MANAGER.lock().await.as_mut() {
        let _ = outbound.key_down(device.to_string(), position).await;
        let _ = outbound.key_up(device.to_string(), position).await;
    }
    true
}
