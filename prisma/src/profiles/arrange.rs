//! Ordena la fila de teclas virtuales de cada perfil: un «Switch Profile» por
//! perfil, con el estilo de Prisma (icono arriba y nombre abajo).
//!
//! Solo se escribe al arrancar y antes de avisar a OpenDeck de que el deck
//! está conectado (que es cuando OpenDeck lee los perfiles). Nunca se toca lo
//! que no es un «Switch Profile», y antes se guarda una copia.

use serde_json::json;

use super::{
    UUID,
    slots::{COUNTS, SWITCH_UUID, Slot, row_of, virtual_keys},
    store::{KEEP_BACKUPS, backup_dir, backup_name, images_dir, list_profiles, profile_path, read_profile, prune_backups, write_profile},
};

/// Si en algún perfil del deck hay una rueda de perfiles
fn has_selector(device: &str) -> bool {
    list_profiles(device).iter().any(|p| {
        read_profile(device, p)
            .and_then(|v| v.get("sliders").cloned())
            .and_then(|s| s.as_array().cloned())
            .is_some_and(|s| s.iter().any(|i| i.pointer("/action/uuid").and_then(|u| u.as_str()) == Some(UUID)))
    })
}

/// Marca de las imágenes que pone Prisma en las teclas virtuales (para
/// cambiarlas sin tocar las que haya puesto el usuario)
const KEY_MARK: &str = "prisma-perfil";

/// Imagen de la tecla virtual de un perfil: su icono de Prisma o el prisma,
/// arriba, dejando abajo sitio para el nombre (lo escribe OpenDeck)
fn key_image(device: &str, target: &str) -> String {
    use base64::Engine;
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    let pic = crate::profile_icons::icon_file(device, target).and_then(|f| {
        let mime = match f.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            _ => return None,
        };
        std::fs::read(&f).ok().map(|b| format!("data:{mime};base64,{}", b64(&b)))
    });
    let body = match pic {
        Some(url) => format!(
            r##"<clipPath id="r"><rect x="34" y="12" width="76" height="76" rx="12"/></clipPath><image x="34" y="12" width="76" height="76" href="{url}" preserveAspectRatio="xMidYMid slice" clip-path="url(#r)"/>"##
        ),
        None => r##"<path d="M72 18L110 84H34z" fill="none" stroke="#3fa9f5" stroke-width="8" stroke-linejoin="round" opacity=".35"/><path d="M72 18L110 84H34z" fill="#0f3b5e" stroke="#7fd0ff" stroke-width="4" stroke-linejoin="round"/><path d="M72 18V62M72 62L34 84M72 62L110 84" stroke="#7fd0ff" stroke-width="3"/>"##.to_string(),
    };
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 144 144" width="144" height="144"><!--{KEY_MARK}--><rect width="144" height="144" fill="#000"/>{body}</svg>"##
    );
    format!("data:image/svg+xml;base64,{}", b64(svg.as_bytes()))
}

/// Si la imagen de una tecla la puso OpenDeck o Prisma (no el usuario)
fn is_default_image(img: &str) -> bool {
    use base64::Engine;
    if img.is_empty() || img.starts_with("plugins/") {
        return true;
    }
    img.strip_prefix("data:image/svg+xml;base64,")
        .and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok())
        .is_some_and(|b| String::from_utf8_lossy(&b).contains(KEY_MARK))
}

/// Pone el estilo de Prisma (imagen y nombre abajo) a un «Switch Profile» de
/// la fila virtual, salvo que el usuario le haya puesto otra imagen
fn restyle(inst: &mut serde_json::Value, device: &str, target: &str) {
    let image = key_image(device, target);
    let name = target.rsplit('/').next().unwrap_or(target);
    for ptr in ["/states/0", "/action/states/0"] {
        if let Some(st) = inst.pointer_mut(ptr)
            && is_default_image(st.get("image").and_then(|i| i.as_str()).unwrap_or(""))
        {
            st["image"] = json!(image);
            st["alignment"] = json!("bottom");
            st["show"] = json!(true);
            if st.get("text").and_then(|t| t.as_str()).is_none_or(|t| t.is_empty()) {
                st["text"] = json!(name);
            }
        }
    }
}

/// Un "Switch Profile" de OpenDeck, igual que los que crea él, con el nombre
/// del perfil como título de la tecla
fn switch_instance(template: &serde_json::Value, position: usize, target: &str) -> serde_json::Value {
    let mut action = template.clone();
    let mut state = action.pointer("/states/0").cloned().unwrap_or_else(|| json!({}));
    state["text"] = json!(target.rsplit('/').next().unwrap_or(target));
    state["show"] = json!(true);
    action["states"] = json!([state.clone()]);
    json!({
        "action": action,
        "children": null,
        "context": format!("Keypad.{position}.0"),
        "current_state": 0,
        "settings": { "profile": target },
        "states": [state],
    })
}

/// La acción "Switch Profile" tal como la guarda OpenDeck: se copia de una que
/// ya exista en algún perfil; si no hay ninguna, la de serie
fn switch_template(device: &str) -> serde_json::Value {
    for p in list_profiles(device) {
        if let Some(v) = read_profile(device, &p)
            && let Some(keys) = v.get("keys").and_then(|k| k.as_array())
            && let Some(a) = keys
                .iter()
                .find(|k| k.pointer("/action/uuid").and_then(|u| u.as_str()) == Some(SWITCH_UUID))
                .and_then(|k| k.get("action"))
        {
            return a.clone();
        }
    }
    let state = json!({
        "alignment": "middle", "background_colour": "#000000", "colour": "#FFFFFF", "family": "Liberation Sans",
        "image": "plugins/com.amansprojects.starterpack.sdPlugin/icons/switchProfile.png", "image_override": null,
        "image_scale": 100, "name": "", "show": true, "size": 16, "stroke_colour": "#000000", "stroke_size": 3,
        "style": "Regular", "text": "", "text_override": null, "underline": false
    });
    json!({
        "controllers": ["Keypad", "Encoder"], "disable_automatic_states": false, "encoder": null,
        "icon": "plugins/com.amansprojects.starterpack.sdPlugin/icons/switchProfile.png", "name": "Switch Profile",
        "plugin": "com.amansprojects.starterpack.sdPlugin",
        "property_inspector": "plugins/com.amansprojects.starterpack.sdPlugin/propertyInspector/switchProfile.html",
        "states": [state], "supported_in_multi_actions": true, "tooltip": "Switch the selected profile",
        "uuid": SWITCH_UUID, "visible_in_action_list": true
    })
}

/// Ordena la fila virtual de cada perfil: un «Switch Profile» por perfil, sin
/// huecos ni repetidos, y quita los de perfiles que ya no existen. Lo que no es
/// un «Switch Profile» se queda donde está. Devuelve (teclas añadidas, cuántas
/// teclas virtuales hacen falta en el deck).
pub(super) fn fill_rows(device: &str, first: usize, n: usize) -> (usize, usize) {
    let profiles = list_profiles(device);
    // Cuántas teclas virtuales se usan ahora (para no esconder nada)
    let used = |v: &serde_json::Value| {
        row_of(v, first, n).iter().rposition(|s| *s != Slot::Empty).map_or(0, |i| i + 1)
    };
    // Solo si hay una rueda de perfiles en el deck o el deck está en los enlaces
    if profiles.is_empty() || !(has_selector(device) || crate::links::is_linked(device)) {
        let need = profiles.iter().filter_map(|p| read_profile(device, p)).map(|v| used(&v)).max().unwrap_or(0);
        return (0, need);
    }
    let template = switch_template(device);
    let mut added = 0;
    let mut need = 0;
    let mut backups: Option<std::path::PathBuf> = None;
    for p in &profiles {
        let Some(path) = profile_path(device, p) else { continue };
        let Some(mut v) = read_profile(device, p) else { continue };
        let row = row_of(&v, first, n);
        let Some(keys) = v.get("keys").and_then(|k| k.as_array()).cloned() else { continue };
        let at = |i: usize| keys.get(first + i).cloned().unwrap_or(serde_json::Value::Null);
        // Los «Switch Profile» que se quedan, en su orden (sin repetidos ni
        // perfiles borrados), y luego los perfiles que falten
        let mut seen: Vec<String> = vec![];
        let mut switches: Vec<(Option<usize>, String)> = vec![];
        for (i, s) in row.iter().enumerate() {
            if let Slot::Switch(t) = s
                && profiles.contains(t)
                && !seen.contains(t)
            {
                seen.push(t.clone());
                switches.push((Some(i), t.clone()));
            }
        }
        for t in profiles.iter().filter(|t| !seen.contains(t)) {
            switches.push((None, t.clone()));
        }
        // Nueva fila: lo que no es «Switch Profile» en su sitio y los demás
        // huecos, por orden, con los «Switch Profile»
        let mut out: Vec<serde_json::Value> = vec![serde_json::Value::Null; n];
        let mut moves: Vec<(usize, usize)> = vec![];
        let mut queue = switches.into_iter();
        for (i, slot) in out.iter_mut().enumerate() {
            if row[i] == Slot::Other {
                *slot = at(i);
                continue;
            }
            let Some((from, target)) = queue.next() else { continue };
            *slot = match from {
                Some(f) => {
                    let mut inst = at(f);
                    inst["context"] = json!(format!("Keypad.{}.0", first + i));
                    if f != i {
                        moves.push((f, i));
                    }
                    restyle(&mut inst, device, &target);
                    inst
                }
                None => {
                    added += 1;
                    let mut inst = switch_instance(&template, first + i, &target);
                    restyle(&mut inst, device, &target);
                    inst
                }
            };
        }
        need = need.max(out.iter().rposition(|x| !x.is_null()).map_or(0, |i| i + 1));
        if (0..n).all(|i| at(i) == out[i]) {
            continue;
        }
        // Copia antes de tocar nada
        if backups.is_none() {
            let Some(dir) = backup_dir(device).filter(|d| std::fs::create_dir_all(d).is_ok()) else {
                log::warn!("Perfiles: no se pudo crear la carpeta de copias; no se toca nada");
                return (0, need.max(n));
            };
            backups = Some(dir);
            prune_backups(KEEP_BACKUPS);
        }
        if std::fs::copy(&path, backups.as_ref().unwrap().join(format!("{}.json", backup_name(p)))).is_err() {
            log::warn!("Perfiles: no se pudo copiar {p}; no se toca");
            need = need.max(used(&v));
            continue;
        }
        let Some(keys) = v.get_mut("keys").and_then(|k| k.as_array_mut()) else { continue };
        for (i, x) in out.into_iter().enumerate() {
            if first + i < keys.len() {
                keys[first + i] = x;
            } else if !x.is_null() {
                keys.resize(first + i + 1, serde_json::Value::Null);
                keys[first + i] = x;
            }
        }
        // Las imágenes propias de una tecla van en una carpeta con su posición
        if let Some(dir) = images_dir(device, p) {
            let tmp: Vec<(std::path::PathBuf, std::path::PathBuf)> = moves
                .iter()
                .filter_map(|(f, t)| {
                    let from = dir.join(format!("Keypad.{}.0", first + f));
                    from.is_dir().then(|| (from, dir.join(format!("Keypad.{}.0", first + t))))
                })
                .collect();
            // Primero a un nombre temporal, por si se cruzan
            for (k, (from, _)) in tmp.iter().enumerate() {
                let _ = std::fs::rename(from, dir.join(format!("prisma-mover-{k}")));
            }
            for (k, (_, to)) in tmp.iter().enumerate() {
                let _ = std::fs::rename(dir.join(format!("prisma-mover-{k}")), to);
            }
        }
        if write_profile(&path, &v).is_err() {
            log::warn!("Perfiles: no se pudo guardar {p}");
        }
    }
    log::info!("Perfiles: fila virtual de {device} ordenada ({added} añadidas, {need} teclas; copia en {:?})", backups);
    (added, need)
}

/// Se llama justo antes de avisar a OpenDeck de que el deck está conectado.
/// Ordena las teclas virtuales de los perfiles y apunta cuántas hacen falta.
/// Devuelve cuántos «Switch Profile» ha añadido.
pub async fn prepare(device: &str, model: &crate::family::Model) -> usize {
    let Some((first, n)) = virtual_keys(model) else { return 0 };
    let d = device.to_string();
    let (added, need) = tokio::task::spawn_blocking(move || fill_rows(&d, first as usize, n as usize)).await.unwrap_or((0, n as usize));
    COUNTS.write().unwrap().insert(device.to_string(), need.min(n as usize) as u8);
    added
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teclas_virtuales_con_estilo_prisma() {
        let mut inst = switch_instance(&switch_template("no-existe"), 13, "aviones/F-18");
        restyle(&mut inst, "no-existe", "aviones/F-18");
        let img = inst.pointer("/states/0/image").and_then(|i| i.as_str()).unwrap().to_string();
        assert!(img.starts_with("data:image/svg+xml") && is_default_image(&img));
        assert_eq!(inst.pointer("/states/0/alignment").and_then(|a| a.as_str()), Some("bottom"));
        assert_eq!(inst.pointer("/states/0/text").and_then(|a| a.as_str()), Some("F-18"));
        // Una imagen puesta por el usuario no se toca
        assert!(!is_default_image("data:image/png;base64,iVBORw0KGgo="));
        inst["states"][0]["image"] = json!("data:image/png;base64,iVBORw0KGgo=");
        restyle(&mut inst, "no-existe", "aviones/F-18");
        assert_eq!(inst.pointer("/states/0/image").and_then(|i| i.as_str()), Some("data:image/png;base64,iVBORw0KGgo="));
    }
}

/// Prueba con copias de perfiles reales: XDG_CONFIG_HOME=carpeta cargo test rellenar_fila -- --ignored --nocapture
#[cfg(test)]
#[test]
#[ignore]
fn rellenar_fila() {
    let dev = "99-C771C7782E1B";
    println!("perfiles: {:?}", list_profiles(dev));
    println!("activo: {:?}", super::store::active_profile(dev));
    println!("añadidas, teclas: {:?}", fill_rows(dev, 10, super::slots::MAX_VIRTUAL as usize));
    for p in list_profiles(dev) {
        println!("{p}: {:?}", row_of(&read_profile(dev, &p).unwrap(), 10, 10));
    }
    println!("la segunda vez: {:?}", fill_rows(dev, 10, 10));
}
