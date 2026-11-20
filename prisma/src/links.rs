//! Perfiles enlazados entre decks.
//!
//! Una tabla: columnas = decks elegidos, filas = enlaces (un perfil por deck, o
//! ninguno = "no cambiar"). Cuando un deck de la tabla cambia a un perfil de un
//! enlace, por la vía que sea (rueda, OpenDeck, tecla "Switch Profile"), los
//! demás decks cambian al perfil de su columna.
//!
//! Se detecta mirando el fichero donde OpenDeck guarda el perfil activo de
//! cada deck. Para cambiar un deck se "pulsa" su tecla virtual con el "Switch
//! Profile" a ese perfil (ver profiles.rs).
//!
//! Se guarda en %APPDATA%\prisma\enlaces.json. Quitar un deck o un enlace solo
//! toca este fichero: los perfiles de OpenDeck no se tocan.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{LazyLock, RwLock},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Links {
    pub enabled: bool,
    /// Decks de la tabla, en orden de columnas
    pub devices: Vec<String>,
    /// Nombre de cada deck (para enseñarlo aunque esté desconectado)
    pub names: HashMap<String, String>,
    /// Cada enlace: deck → perfil (sin clave o null = no cambiar)
    pub links: Vec<HashMap<String, Option<String>>>,
}

static LINKS: LazyLock<RwLock<Links>> = LazyLock::new(|| RwLock::new(load()));

fn path() -> Option<std::path::PathBuf> {
    Some(crate::app_dir()?.join("enlaces.json"))
}

fn load() -> Links {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn get() -> Links {
    LINKS.read().unwrap().clone()
}

/// Si el deck está en la tabla de enlaces (entonces lleva teclas virtuales con
/// sus perfiles)
pub fn is_linked(device: &str) -> bool {
    let l = LINKS.read().unwrap();
    l.enabled && l.devices.iter().any(|d| d == device)
}

/// Guarda la tabla, limpia: sin enlaces de decks que ya no están, y un mismo
/// perfil solo en un enlace por deck (para que no haya cambios en cadena)
pub fn save(mut l: Links) -> Result<Links, String> {
    let devices = l.devices.clone();
    l.names.retain(|d, _| devices.contains(d));
    let mut used: HashMap<String, Vec<String>> = HashMap::new();
    for link in &mut l.links {
        link.retain(|d, p| devices.contains(d) && p.as_ref().is_some_and(|p| !p.is_empty()));
        for (d, p) in link.iter_mut() {
            let seen = used.entry(d.clone()).or_default();
            if let Some(name) = p.clone() {
                if seen.contains(&name) {
                    *p = None;
                } else {
                    seen.push(name);
                }
            }
        }
        link.retain(|_, p| p.is_some());
    }
    l.links.retain(|link| !link.is_empty());
    let p = path().ok_or(crate::i18n::t("error.no_config_folder"))?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(&l).map_err(|e| e.to_string())?;
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &p)).map_err(|e| e.to_string())?;
    *LINKS.write().unwrap() = l.clone();
    Ok(l)
}

/// Un perfil de un deck ha cambiado de nombre: sus enlaces lo siguen
pub fn rename_profile(device: &str, old: &str, new: &str) -> Result<(), String> {
    let mut l = get();
    let mut changed = false;
    for p in l.links.iter_mut().filter_map(|link| link.get_mut(device)) {
        if p.as_deref() == Some(old) {
            *p = Some(new.to_string());
            changed = true;
        }
    }
    if changed {
        save(l)?;
    }
    Ok(())
}

// ------------------------------------------------------ decks conocidos --

fn known_path() -> Option<std::path::PathBuf> {
    Some(crate::app_dir()?.join("decks.json"))
}

/// Apunta el nombre de un deck al conectarse, para enseñarlo aunque luego se
/// desconecte
pub fn remember_deck(id: &str, name: &str) {
    let Some(p) = known_path() else { return };
    let mut known: HashMap<String, String> =
        std::fs::read_to_string(&p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if known.get(id).map(String::as_str) == Some(name) {
        return;
    }
    known.insert(id.to_string(), name.to_string());
    let _ = std::fs::create_dir_all(p.parent().unwrap());
    if let Ok(text) = serde_json::to_string_pretty(&known) {
        let _ = std::fs::write(p, text);
    }
}

/// Todos los decks de Prisma que tienen perfiles en OpenDeck (conectados o no),
/// con el nombre que se conozca
pub fn all_decks() -> Vec<(String, Option<String>)> {
    let known: HashMap<String, String> = known_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let mut ids: Vec<String> = known.keys().cloned().collect();
    if let Some(dir) = dirs::config_dir().map(|d| d.join("opendeck").join("profiles"))
        && let Ok(entries) = std::fs::read_dir(dir)
    {
        for e in entries.flatten() {
            let id = e.file_name().to_string_lossy().into_owned();
            // Solo los decks de Prisma (los de otros plugins tienen otro prefijo)
            if e.path().is_dir() && id.starts_with("99-") && !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids.into_iter().map(|id| { let n = known.get(&id).cloned(); (id, n) }).collect()
}

/// Qué decks cambiar cuando `device` pasa a `profile`
pub fn targets(l: &Links, device: &str, profile: &str) -> Vec<(String, String)> {
    let Some(link) = l.links.iter().find(|k| k.get(device).and_then(|p| p.as_deref()) == Some(profile)) else {
        return vec![];
    };
    l.devices
        .iter()
        .filter(|d| *d != device)
        .filter_map(|d| Some((d.clone(), link.get(d)?.clone()?)))
        .collect()
}

/// Vigila el perfil activo de los decks de la tabla y cambia los enlazados
pub async fn watch_task() {
    let mut last: HashMap<String, String> = HashMap::new();
    // Cambios que ha hecho Prisma: no se reenvían (si no, rebotarían)
    let mut expected: HashMap<String, (String, Instant)> = HashMap::new();
    loop {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let l = get();
        if !l.enabled || l.devices.len() < 2 {
            last.clear();
            continue;
        }
        let connected: Vec<String> = crate::DEVICES.read().await.keys().cloned().collect();
        let devices: Vec<String> = l.devices.iter().filter(|d| connected.contains(d)).cloned().collect();
        let current: Vec<(String, Option<String>)> = {
            let ds = devices.clone();
            tokio::task::spawn_blocking(move || ds.into_iter().map(|d| {
                let p = crate::profiles::active_profile(&d);
                (d, p)
            }).collect())
            .await
            .unwrap_or_default()
        };
        expected.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(5));
        for (device, profile) in current {
            let Some(profile) = profile else { continue };
            let before = last.insert(device.clone(), profile.clone());
            // La primera vez solo se apunta; luego, solo los cambios
            if before.is_none() || before.as_deref() == Some(profile.as_str()) {
                continue;
            }
            if expected.get(&device).is_some_and(|(p, _)| *p == profile) {
                expected.remove(&device);
                continue;
            }
            log::info!("Enlaces: {device} ha pasado a «{profile}»");
            for (other, target) in targets(&l, &device, &profile) {
                if !connected.contains(&other) || last.get(&other) == Some(&target) {
                    continue;
                }
                expected.insert(other.clone(), (target.clone(), Instant::now()));
                if crate::profiles::switch_device(&other, &target).await {
                    log::info!("Enlaces: {other} → «{target}»");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(pairs: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
        pairs.iter().map(|(d, p)| (d.to_string(), p.map(String::from))).collect()
    }

    #[test]
    fn a_quien_cambiar() {
        let l = Links {
            enabled: true,
            devices: vec!["pro".into(), "one".into(), "pro2".into()],
            names: HashMap::new(),
            links: vec![
                link(&[("pro", Some("Default")), ("one", Some("Default")), ("pro2", Some("Default"))]),
                link(&[("pro", Some("aviones")), ("pro2", Some("Juegos"))]),
            ],
        };
        let mut t = targets(&l, "pro", "aviones");
        t.sort();
        assert_eq!(t, vec![("pro2".into(), "Juegos".into())]);
        assert_eq!(targets(&l, "pro2", "Juegos"), vec![("pro".into(), "aviones".into())]);
        assert!(targets(&l, "one", "CPU").is_empty());
        assert_eq!(targets(&l, "one", "Default").len(), 2);
    }
}
