//! Los ficheros de perfiles de OpenDeck.
//!
//! OpenDeck guarda cada perfil en %APPDATA%\opendeck\profiles\<deck>\<perfil>.json
//! (los subperfiles, en una carpeta) y el perfil activo en
//! %APPDATA%\opendeck\profiles\<deck>.json. Las imágenes propias de las teclas
//! van en %APPDATA%\opendeck\images\<deck>\<perfil>. Aquí se leen y se
//! escriben; antes de tocar nada se guarda una copia en %APPDATA%\prisma\copias.

use std::path::{Path, PathBuf};

fn profiles_dir() -> Option<std::path::PathBuf> {
    Some(dirs::config_dir()?.join("opendeck").join("profiles"))
}

/// Perfiles de un deck ("Default" primero, luego por orden alfabético)
pub fn list_profiles(device: &str) -> Vec<String> {
    let Some(dir) = profiles_dir().map(|d| d.join(device)) else { return vec![] };
    let mut out = vec![];
    let mut add = |id: String| {
        if !out.contains(&id) {
            out.push(id);
        }
    };
    let Ok(entries) = std::fs::read_dir(&dir) else { return vec![] };
    for e in entries.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if path.is_file() {
            if let Some(id) = name.strip_suffix(".json") {
                add(id.to_string());
            }
        } else if path.is_dir()
            && let Ok(sub) = std::fs::read_dir(&path)
        {
            for f in sub.flatten() {
                let fname = f.file_name().to_string_lossy().into_owned();
                if let Some(id) = fname.strip_suffix(".json") {
                    add(format!("{name}/{id}"));
                }
            }
        }
    }
    out.sort_by_key(|a| (a != "Default", a.to_lowercase()));
    out
}

pub(super) fn profile_path(device: &str, profile: &str) -> Option<std::path::PathBuf> {
    Some(profiles_dir()?.join(device).join(format!("{profile}.json")))
}

pub(super) fn read_profile(device: &str, profile: &str) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(profile_path(device, profile)?).ok()?).ok()
}

/// Perfil activo de un deck, según OpenDeck
pub fn active_profile(device: &str) -> Option<String> {
    let path = profiles_dir()?.join(format!("{device}.json"));
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    v.get("selected_profile")?.as_str().map(String::from)
}

/// Si alguna acción de algún perfil del deck tiene `key` = `value` en sus
/// ajustes (también dentro de las multiacciones)
pub fn setting_in_profiles(device: &str, key: &str, value: &str) -> bool {
    list_profiles(device).iter().filter_map(|p| read_profile(device, p)).any(|v| has_setting(&v, key, value))
}

fn has_setting(v: &serde_json::Value, key: &str, value: &str) -> bool {
    match v {
        serde_json::Value::Object(map) => {
            map.get("settings").and_then(|s| s.get(key)).and_then(|x| x.as_str()) == Some(value) || map.values().any(|x| has_setting(x, key, value))
        }
        serde_json::Value::Array(a) => a.iter().any(|x| has_setting(x, key, value)),
        _ => false,
    }
}

/// Cambia el perfil activo que OpenDeck lee al conectar el deck
pub(super) fn set_active_profile(device: &str, name: &str) -> std::io::Result<()> {
    let path = profiles_dir().ok_or(std::io::ErrorKind::NotFound)?.join(format!("{device}.json"));
    let mut v: serde_json::Value = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
    v["selected_profile"] = serde_json::json!(name);
    write_profile(&path, &v)
}

/// Carpeta de las imágenes propias de las teclas de un perfil
pub(super) fn images_dir(device: &str, profile: &str) -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("opendeck").join("images").join(device).join(profile.replace('/', std::path::MAIN_SEPARATOR_STR)))
}

/// Carpeta nueva para las copias de un deck: %APPDATA%\prisma\copias\<fecha>\<deck>
pub(super) fn backup_dir(device: &str) -> Option<PathBuf> {
    Some(crate::app_dir()?.join("copias").join(chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()).join(device))
}

/// Copias que se guardan: las más recientes; las viejas se borran solas
pub(super) const KEEP_BACKUPS: usize = 5;

/// Deja solo las `keep` copias más recientes (las carpetas con fecha de
/// %APPDATA%\prisma\copias)
pub(super) fn prune_backups(keep: usize) {
    if let Some(dir) = crate::app_dir().map(|d| d.join("copias")) {
        prune_dir(&dir, keep);
    }
}

/// Borra las subcarpetas más viejas de `dir` (por nombre: la fecha ordena
/// sola) hasta dejar `keep`
fn prune_dir(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut dated: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dated.sort();
    let extra = dated.len().saturating_sub(keep);
    for old in &dated[..extra] {
        if let Err(e) = std::fs::remove_dir_all(old) {
            log::warn!("Perfiles: no se pudo borrar la copia vieja {}: {e}", old.display());
        }
    }
}

/// Nombre de fichero de un perfil para las copias ("aviones/F-18" → "aviones__F-18")
pub(super) fn backup_name(profile: &str) -> String {
    profile.replace('/', "__")
}

/// Guarda un perfil: primero en un temporal y luego se cambia el nombre, así
/// nunca queda a medias
pub(super) fn write_profile(path: &Path, v: &serde_json::Value) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(v).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("prisma-tmp");
    let r = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, path));
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r
}

pub(super) fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn busca_un_ajuste() {
        let v = serde_json::json!({ "keys": [null, { "settings": {} , "children": [{ "settings": { "uid": "t-1" } }] }], "sliders": [{ "settings": { "uid": "t-2" } }] });
        assert!(super::has_setting(&v, "uid", "t-1") && super::has_setting(&v, "uid", "t-2"));
        assert!(!super::has_setting(&v, "uid", "t-3"));
    }

    #[test]
    fn solo_las_ultimas_copias() {
        let dir = std::env::temp_dir().join(format!("prisma-copias-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["2026-11-01_10-00-00", "2026-11-03_10-00-00", "2026-11-02_10-00-00", "2026-11-04_10-00-00"] {
            std::fs::create_dir_all(dir.join(d).join("deck")).unwrap();
        }
        super::prune_dir(&dir, 2);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, vec!["2026-11-03_10-00-00", "2026-11-04_10-00-00"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
