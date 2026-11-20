//! Crear, renombrar y borrar perfiles desde el panel.
//!
//! Hay que llamarlo con el deck desconectado de OpenDeck (ver
//! `device::reload_profiles`): si no, OpenDeck pisaría los cambios.

use serde_json::json;

use super::{
    slots::SWITCH_UUID,
    store::{
        active_profile, copy_dir, images_dir, list_profiles, profile_path, read_profile, set_active_profile, write_profile,
    },
};

/// Nombre válido para un perfil nuevo (es también el nombre del fichero)
pub fn check_name(device: &str, name: &str) -> Result<String, String> {
    let name = valid_name(name)?;
    if list_profiles(device).iter().any(|p| p.to_lowercase() == name.to_lowercase()) {
        return Err(crate::i18n::tf("error.profile_exists", &[&name]));
    }
    Ok(name)
}

/// Lo que tiene que cumplir el nombre de un perfil, que es también el nombre
/// de su fichero en Windows
fn valid_name(name: &str) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(crate::i18n::t("error.profile_name_empty").into());
    }
    if name.chars().count() > 40 {
        return Err(crate::i18n::t("error.profile_name_long").into());
    }
    if name.starts_with('.') || name.ends_with('.') || name.chars().any(|c| "/\\:*?\"<>|".contains(c) || c.is_control()) {
        return Err(crate::i18n::t("error.profile_name_chars").into());
    }
    let upper = name.to_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "LPT1", "LPT2"].contains(&upper.as_str()) {
        return Err(crate::i18n::t("error.profile_name_reserved").into());
    }
    Ok(name)
}

/// Crea el fichero del perfil: vacío o copia de otro (con sus imágenes). Sus
/// «Switch Profile» los pone `prepare` al volver a conectar el deck.
/// Hay que llamarlo con el deck desconectado de OpenDeck (ver
/// `device::reload_profiles`), si no OpenDeck pisaría los cambios.
pub fn create_profile(device: &str, model: &crate::family::Model, name: &str, copy_of: Option<&str>) -> Result<(), String> {
    let name = check_name(device, name)?;
    let path = profile_path(device, &name).ok_or(crate::i18n::t("error.profiles_folder"))?;
    let l = model.layout();
    let value = match copy_of {
        Some(src) => {
            let v = read_profile(device, src).ok_or(crate::i18n::tf("error.profile_read", &[&src]))?;
            // Las imágenes propias de las teclas van en images\<deck>\<perfil>
            if let (Some(from), Some(to)) = (images_dir(device, src), images_dir(device, &name))
                && from.is_dir()
            {
                copy_dir(&from, &to).map_err(|e| crate::i18n::tf("error.copy_images", &[&e]))?;
            }
            v
        }
        None => json!({
            "keys": vec![serde_json::Value::Null; (l.rows * l.cols + l.virtual_rows * l.cols) as usize],
            "sliders": vec![serde_json::Value::Null; l.encoders],
            "infobars": [],
        }),
    };
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    write_profile(&path, &value).map_err(|e| e.to_string())?;
    log::info!("Perfiles: creado «{name}» en {device}{}", copy_of.map(|c| format!(" (copia de «{c}»)")).unwrap_or_default());
    Ok(())
}

/// Si un perfil se puede renombrar (no «Default») y el nombre nuevo vale. Se
/// puede cambiar solo mayúsculas y minúsculas. Devuelve el nombre nuevo limpio.
pub fn check_rename(device: &str, old: &str, new: &str) -> Result<String, String> {
    if !list_profiles(device).iter().any(|p| p == old) {
        return Err(crate::i18n::tf("error.profile_read", &[&old]));
    }
    if old == "Default" {
        return Err(crate::i18n::t("error.profile_rename_default").into());
    }
    let new = valid_name(new)?;
    if new != old && list_profiles(device).iter().any(|p| p != old && p.to_lowercase() == new.to_lowercase()) {
        return Err(crate::i18n::tf("error.profile_exists", &[&new]));
    }
    Ok(new)
}

/// Cambia el nombre de un perfil, también en OpenDeck: su fichero, la carpeta
/// de sus imágenes, las teclas que saltan a él («Switch Profile», «Entrar /
/// cambiar» y «Perfil actual») en todos los perfiles del deck y el perfil
/// activo si es él. En Prisma, su icono, su sitio en la jerarquía y los
/// enlaces entre decks.
/// Igual que al crear, con el deck desconectado de OpenDeck (`device::reload_profiles`).
pub fn rename_profile(device: &str, old: &str, new: &str) -> Result<(), String> {
    let new = check_rename(device, old, new)?;
    if new == old {
        return Ok(());
    }
    let from = profile_path(device, old).ok_or(crate::i18n::t("error.profiles_folder"))?;
    let to = profile_path(device, &new).ok_or(crate::i18n::t("error.profiles_folder"))?;
    // Las teclas que saltan al perfil, en todos los perfiles del deck
    for p in list_profiles(device) {
        let (Some(path), Some(mut v)) = (profile_path(device, &p), read_profile(device, &p)) else { continue };
        if retarget(&mut v, old, &new) {
            write_profile(&path, &v).map_err(|e| e.to_string())?;
        }
    }
    // El perfil y sus imágenes
    std::fs::rename(&from, &to).map_err(|e| e.to_string())?;
    if let (Some(a), Some(b)) = (images_dir(device, old), images_dir(device, &new))
        && a.is_dir()
    {
        std::fs::rename(&a, &b).map_err(|e| e.to_string())?;
    }
    if active_profile(device).as_deref() == Some(old) {
        set_active_profile(device, &new).map_err(|e| e.to_string())?;
    }
    // Lo de Prisma
    crate::profile_icons::rename(device, old, &new)?;
    crate::links::rename_profile(device, old, &new)?;
    log::info!("Perfiles: «{old}» ahora se llama «{new}» en {device}");
    Ok(())
}

/// Las acciones que guardan un perfil de destino en sus ajustes (`profile`)
fn points_to_profile(uuid: &str) -> bool {
    uuid == SWITCH_UUID || uuid == crate::profile_keys::GO || uuid == crate::profile_keys::NOW
}

/// Cambia el destino de las teclas que saltan a `old` (también dentro de las
/// multiacciones) y el título de los «Switch Profile» que llevan su nombre.
/// Dice si ha cambiado algo.
fn retarget(v: &mut serde_json::Value, old: &str, new: &str) -> bool {
    let mut changed = false;
    match v {
        serde_json::Value::Object(map) => {
            let uuid = map.get("action").and_then(|a| a.get("uuid")).and_then(|u| u.as_str()).unwrap_or("").to_string();
            if points_to_profile(&uuid) && map.get("settings").and_then(|s| s.get("profile")).and_then(|p| p.as_str()) == Some(old) {
                map["settings"]["profile"] = json!(new);
                changed = true;
                // El título que pone Prisma (o OpenDeck) con el nombre del perfil
                if uuid == SWITCH_UUID {
                    let (old_t, new_t) = (old.rsplit('/').next().unwrap_or(old), new.rsplit('/').next().unwrap_or(new));
                    retitle(map.get_mut("states"), old_t, new_t);
                    retitle(map.get_mut("action").and_then(|a| a.get_mut("states")), old_t, new_t);
                }
            }
            for (k, x) in map.iter_mut() {
                if k != "action" && k != "settings" {
                    changed |= retarget(x, old, new);
                }
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                changed |= retarget(x, old, new);
            }
        }
        _ => {}
    }
    changed
}

/// Cambia el título de los estados que llevan el nombre viejo
fn retitle(states: Option<&mut serde_json::Value>, old: &str, new: &str) {
    for st in states.and_then(|s| s.as_array_mut()).into_iter().flatten() {
        if st.get("text").and_then(|t| t.as_str()) == Some(old) {
            st["text"] = json!(new);
        }
    }
}

/// Si un perfil se puede borrar (con sus subperfiles si `with_children`): ni
/// el activo, ni «Default», ni todos los que quedan. Devuelve los perfiles
/// que se borrarían, primero los de más adentro.
pub fn check_delete(device: &str, name: &str, with_children: bool) -> Result<Vec<String>, String> {
    let all = list_profiles(device);
    if !all.iter().any(|p| p == name) {
        return Err(crate::i18n::tf("error.profile_read", &[&name]));
    }
    let tree = crate::profile_icons::tree(device, &all);
    // Los de más adentro primero, el perfil al final
    let mut gone: Vec<String> = if with_children {
        let mut inside: Vec<(String, usize)> = crate::hierarchy::flatten(&tree).into_iter().filter(|(p, _)| crate::hierarchy::is_inside(&tree, p, name)).collect();
        inside.sort_by_key(|(_, depth)| std::cmp::Reverse(*depth));
        inside.into_iter().map(|(p, _)| p).collect()
    } else {
        vec![]
    };
    gone.push(name.to_string());
    let active = active_profile(device);
    for g in &gone {
        let inside = g != name;
        if g == "Default" {
            return Err(crate::i18n::t(if inside { "error.profile_delete_default_inside" } else { "error.profile_delete_default" }).into());
        }
        if active.as_deref() == Some(g.as_str()) {
            return Err(crate::i18n::t(if inside { "error.profile_delete_active_inside" } else { "error.profile_delete_active" }).into());
        }
    }
    if all.len() <= gone.len() {
        return Err(crate::i18n::t("error.profile_delete_last").into());
    }
    Ok(gone)
}

/// Borra un perfil y sus imágenes. Sus subperfiles se borran también
/// (`with_children`) o suben al nivel del perfil borrado. Igual que al crear,
/// hay que llamarlo con el deck desconectado de OpenDeck (`device::reload_profiles`).
pub fn delete_profile(device: &str, name: &str, with_children: bool) -> Result<(), String> {
    let gone = check_delete(device, name, with_children)?;
    // Dónde estaba en la jerarquía, antes de quitarlo
    let before = crate::profile_icons::tree(device, &list_profiles(device));
    let parent = crate::hierarchy::parent_of(&before, name);
    let kids = crate::hierarchy::children(&before, Some(name));
    for g in &gone {
        let path = profile_path(device, g).ok_or(crate::i18n::t("error.profiles_folder"))?;
        if let Some(images) = images_dir(device, g).filter(|d| d.is_dir()) {
            std::fs::remove_dir_all(&images).map_err(|e| e.to_string())?;
        }
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        let _ = crate::profile_icons::set_icon(device, g, None);
    }
    // Si se quedan, sus subperfiles pasan a colgar de su padre
    let mut tree = crate::profile_icons::tree(device, &list_profiles(device));
    if !with_children {
        for k in kids {
            let _ = crate::hierarchy::set_parent(&mut tree, &k, parent.as_deref());
        }
    }
    let _ = crate::profile_icons::save_tree(device, tree);
    log::info!("Perfiles: borrados {gone:?} en {device}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teclas_que_siguen_al_perfil() {
        let switch = |to: &str, text: &str| {
            json!({ "action": { "uuid": SWITCH_UUID, "states": [{ "text": text }] }, "settings": { "profile": to }, "states": [{ "text": text }] })
        };
        let mut v = json!({
            "keys": [
                switch("Juegos", "Juegos"),
                switch("Juegos", "Mis juegos"),
                switch("Otro", "Otro"),
                { "action": { "uuid": crate::profile_keys::GO }, "settings": { "profile": "Juegos" } },
                // Dentro de una multiacción
                { "action": { "uuid": "multi" }, "settings": {}, "children": [switch("Juegos", "Juegos")] },
                null
            ]
        });
        assert!(retarget(&mut v, "Juegos", "Coches"));
        let k = &v["keys"];
        assert_eq!(k[0]["settings"]["profile"], "Coches");
        assert_eq!(k[0]["states"][0]["text"], "Coches");
        assert_eq!(k[0]["action"]["states"][0]["text"], "Coches");
        // Un título puesto a mano se respeta
        assert_eq!(k[1]["settings"]["profile"], "Coches");
        assert_eq!(k[1]["states"][0]["text"], "Mis juegos");
        assert_eq!(k[2]["settings"]["profile"], "Otro");
        assert_eq!(k[3]["settings"]["profile"], "Coches");
        assert_eq!(k[4]["children"][0]["settings"]["profile"], "Coches");
        assert!(!retarget(&mut v, "Juegos", "Coches"));
    }

    #[test]
    fn nombres_validos() {
        assert_eq!(valid_name("  Coches ").unwrap(), "Coches");
        assert!(valid_name("a/b").is_err() && valid_name("").is_err() && valid_name("CON").is_err());
    }

    /// Renombrar y borrar con perfiles de verdad en una carpeta de pruebas
    /// (cambia XDG_CONFIG_HOME, así que va solo): cargo test perfiles_en_disco -- --ignored
    #[test]
    #[ignore]
    fn perfiles_en_disco() {
        let root = std::env::temp_dir().join(format!("prisma-renombrar-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // SAFETY: esta prueba va sola (--ignored), nadie más lee el entorno a la vez
        unsafe { std::env::set_var("XDG_CONFIG_HOME", &root) };
        let dev = "99-TEST";
        let dir = root.join("opendeck/profiles").join(dev);
        std::fs::create_dir_all(&dir).unwrap();
        let profile = |keys: serde_json::Value| json!({ "keys": keys, "sliders": [], "infobars": [] });
        let sw = |to: &str| json!({ "action": { "uuid": SWITCH_UUID, "states": [{ "text": to }] }, "settings": { "profile": to }, "states": [{ "text": to }] });
        std::fs::write(dir.join("Default.json"), profile(json!([sw("Juegos")])).to_string()).unwrap();
        std::fs::write(dir.join("Juegos.json"), profile(json!([sw("Default")])).to_string()).unwrap();
        std::fs::write(root.join("opendeck/profiles").join(format!("{dev}.json")), r#"{"selected_profile":"Juegos"}"#).unwrap();
        let img = root.join("opendeck/images").join(dev).join("Juegos/Keypad.0.0");
        std::fs::create_dir_all(&img).unwrap();
        std::fs::write(img.join("0.png"), b"png").unwrap();

        assert!(check_rename(dev, "Default", "X").is_err());
        assert!(check_rename(dev, "Juegos", "default").is_err());
        assert_eq!(check_rename(dev, "Juegos", "JUEGOS").unwrap(), "JUEGOS");
        rename_profile(dev, "Juegos", "Coches").unwrap();

        assert_eq!(list_profiles(dev), vec!["Default", "Coches"]);
        assert_eq!(active_profile(dev).as_deref(), Some("Coches"));
        assert_eq!(read_profile(dev, "Default").unwrap()["keys"][0]["settings"]["profile"], "Coches");
        assert!(root.join("opendeck/images").join(dev).join("Coches/Keypad.0.0/0.png").is_file());
        assert!(!root.join("opendeck/images").join(dev).join("Juegos").exists());

        // Borrar con subperfiles: subirlos un nivel o borrarlo todo
        let new = |name: &str, parent: Option<&str>| {
            std::fs::write(dir.join(format!("{name}.json")), profile(json!([])).to_string()).unwrap();
            let mut tree = crate::profile_icons::tree(dev, &list_profiles(dev));
            crate::hierarchy::set_parent(&mut tree, name, parent).unwrap();
            crate::profile_icons::save_tree(dev, tree).unwrap();
        };
        let parent = |name: &str| crate::hierarchy::parent_of(&crate::profile_icons::tree(dev, &list_profiles(dev)), name);
        new("A", None);
        new("A1", Some("A"));
        new("A2", Some("A1"));
        assert_eq!(check_delete(dev, "A", false).unwrap(), vec!["A"]);
        assert_eq!(check_delete(dev, "A", true).unwrap(), vec!["A2", "A1", "A"]);
        delete_profile(dev, "A", false).unwrap();
        assert_eq!(parent("A1"), None);
        assert_eq!(parent("A2").as_deref(), Some("A1"));
        delete_profile(dev, "A1", true).unwrap();
        assert_eq!(list_profiles(dev), vec!["Default", "Coches"]);
        // Con el activo dentro no se puede borrar todo (sí subir los subniveles)
        new("B", None);
        let mut tree = crate::profile_icons::tree(dev, &list_profiles(dev));
        crate::hierarchy::set_parent(&mut tree, "Coches", Some("B")).unwrap();
        crate::profile_icons::save_tree(dev, tree).unwrap();
        assert!(check_delete(dev, "B", true).is_err());
        assert!(check_delete(dev, "B", false).is_ok());
        // Ninguna copia
        assert!(!root.join("prisma/copias").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Crear perfiles con copias de perfiles reales:
    /// XDG_CONFIG_HOME=carpeta cargo test crear_perfil -- --ignored --nocapture
    #[cfg(test)]
    #[test]
    #[ignore]
    fn crear_perfil() {
        let dev = "99-C771C7782E1B";
        let model = crate::family::Model::from_vid_pid(0x0B00, 0x1003).expect("MSD-PRO");
        assert!(check_name(dev, "coches").is_err());
        assert!(check_name(dev, "COCHES").is_err());
        assert!(check_name(dev, "a/b").is_err());
        assert!(check_name(dev, "  ").is_err());
        create_profile(dev, &model, "Juegos", None).unwrap();
        create_profile(dev, &model, "coches 2", Some("coches")).unwrap();
        println!("ordenar: {:?}", super::super::arrange::fill_rows(dev, 10, 10));
        println!("perfiles: {:?}", list_profiles(dev));
        for p in list_profiles(dev) {
            println!("{p}: {:?}", super::super::slots::row_of(&read_profile(dev, &p).unwrap(), 10, 10));
        }
    }
}
