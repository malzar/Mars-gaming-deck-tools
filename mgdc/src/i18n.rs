//! Idiomas. Todos los textos están en `locales/<idioma>.json` (es.json,
//! en.json…): el plugin usa la sección `plugin`, las ventanas de ajustes la
//! sección `pi`, el panel la sección `panel` y OpenDeck la sección `opendeck`
//! (para los nombres de las acciones).
//!
//! En el código solo hay claves: `t("weather.uv.title")`, o `tf` para textos
//! con datos (`{}` en el fichero se cambia por cada dato, en orden).
//!
//! Un texto se busca en el idioma de OpenDeck, luego en inglés y luego en
//! español. El español y el inglés van dentro del programa; cualquier otro
//! idioma se lee de la carpeta `locales` del plugin, así que para añadir uno
//! basta con poner su fichero, sin compilar.

use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
};

const ES: &str = include_str!("../locales/es.json");
const EN: &str = include_str!("../locales/en.json");

type Table = HashMap<String, &'static str>;

thread_local! {
    /// Idioma elegido a mano en este hilo (pruebas); si no, el de OpenDeck
    static FORCED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Código de idioma que pasa OpenDeck al arrancar el plugin («-info»): "es",
/// "en", "zh_CN"…
pub fn code() -> String {
    if let Some(c) = FORCED.with(|f| f.borrow().clone()) {
        return c;
    }
    static CODE: LazyLock<String> = LazyLock::new(|| {
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|a| a.eq_ignore_ascii_case("-info"))
            .and_then(|i| args.get(i + 1))
            .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
            .and_then(|v| v.pointer("/application/language").and_then(|l| l.as_str()).map(String::from))
            .unwrap_or_else(|| "es".into())
    });
    CODE.clone()
}

#[cfg(test)]
pub fn force(code: Option<&str>) {
    FORCED.with(|f| *f.borrow_mut() = code.map(String::from));
}

/// Carpeta `locales` junto al programa (dentro del plugin instalado)
fn folder() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.parent()?.join("locales"))
}

/// El fichero de un idioma: el de la carpeta del plugin y, si no está, el
/// que va dentro del programa
pub fn file(code: &str) -> Option<serde_json::Value> {
    let from_disk = folder()
        .map(|d| d.join(format!("{code}.json")))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok());
    from_disk.or_else(|| match code {
        "es" => serde_json::from_str(ES).ok(),
        "en" => serde_json::from_str(EN).ok(),
        _ => None,
    })
}

/// Idiomas en los que buscar un texto, en orden: el pedido ("zh_CN", y
/// también "zh"), inglés y español
pub fn chain(code: &str) -> Vec<String> {
    let mut v = vec![code.to_string()];
    if let Some((base, _)) = code.split_once(['_', '-']) {
        v.push(base.to_string());
    }
    v.push("en".into());
    v.push("es".into());
    v.dedup();
    let mut out: Vec<String> = vec![];
    for c in v {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

/// Una sección ("plugin", "pi", "panel") con todo lo de la cadena de idiomas
/// ya mezclado: lo que falte en un idioma sale del siguiente
pub fn section(code: &str, name: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut out = serde_json::Map::new();
    for c in chain(code).iter().rev() {
        if let Some(serde_json::Value::Object(m)) = file(c).and_then(|f| f.get(name).cloned()) {
            out.extend(m);
        }
    }
    out
}

/// Tabla de textos del plugin para un idioma (se hace una vez por idioma)
fn table(code: &str) -> Arc<Table> {
    static CACHE: LazyLock<Mutex<HashMap<String, Arc<Table>>>> = LazyLock::new(Default::default);
    if let Some(t) = CACHE.lock().unwrap().get(code) {
        return t.clone();
    }
    let t: Table = section(code, "plugin")
        .into_iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k, &*Box::leak(s.to_string().into_boxed_str()))))
        .collect();
    let t = Arc::new(t);
    CACHE.lock().unwrap().insert(code.to_string(), t.clone());
    t
}

/// El texto de una clave en el idioma de OpenDeck (si no existe, la clave)
pub fn t(key: &'static str) -> &'static str {
    table(&code()).get(key).copied().unwrap_or(key)
}

/// Igual, rellenando cada `{}` con un dato
pub fn tf(key: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut parts = t(key).split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for (i, p) in parts.enumerate() {
        if let Some(a) = args.get(i) {
            out.push_str(&a.to_string());
        }
        out.push_str(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys(code: &str, sec: &str) -> BTreeSet<String> {
        file(code).unwrap()[sec].as_object().unwrap().keys().cloned().collect()
    }

    #[test]
    fn traduce() {
        force(Some("en"));
        assert_eq!(t("moon.first_quarter"), "First quarter");
        assert_eq!(t("no.existe"), "no.existe");
        assert_eq!(tf("error.profile_exists", &[&"Juegos"]), "There's already a profile called «Juegos»");
        force(Some("es"));
        assert_eq!(tf("weather.sun.daylight", &[&9, &"05"]), "9 h 05 min de luz");
        // Un idioma sin fichero sale en inglés
        force(Some("fr"));
        assert_eq!(t("weather.today"), "TODAY");
        force(None);
        assert_eq!(chain("zh_CN"), vec!["zh_CN", "zh", "en", "es"]);
    }

    /// Todos los idiomas tienen las mismas claves que el español
    #[test]
    fn ficheros_completos() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let p = entry.unwrap().path();
            let code = p.file_stem().unwrap().to_str().unwrap().to_string();
            let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            for sec in ["plugin", "pi", "panel", "opendeck"] {
                let have: BTreeSet<String> = v[sec].as_object().unwrap_or_else(|| panic!("{code}: falta {sec}")).keys().cloned().collect();
                let want = keys("es", sec);
                let missing: Vec<_> = want.difference(&have).collect();
                let extra: Vec<_> = have.difference(&want).collect();
                assert!(missing.is_empty() && extra.is_empty(), "{code}.json/{sec}: faltan {missing:?}, sobran {extra:?}");
            }
        }
    }

    /// Cada clave que usa el código existe en los ficheros (y no sobra ninguna)
    #[test]
    fn claves_usadas() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let plugin = keys("es", "plugin");
        let mut used = BTreeSet::new();
        // Todo lo que entre dos comillas parezca una clave («weather.uv.title»)
        let key_re = |s: &str| {
            let quotes: Vec<usize> = s.match_indices('"').map(|(i, _)| i).collect();
            quotes
                .windows(2)
                .map(|w| &s[w[0] + 1..w[1]])
                .filter(|lit| {
                    lit.contains('.')
                        && lit.starts_with(|c: char| c.is_ascii_lowercase())
                        && lit.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'_')
                })
                .map(String::from)
                .collect::<Vec<_>>()
        };
        for p in rust_files(&root.join("src")) {
            if p.file_name().unwrap() != "i18n.rs" {
                for k in key_re(&std::fs::read_to_string(&p).unwrap()) {
                    if plugin.iter().any(|x| x.split('.').next() == k.split('.').next()) {
                        used.insert(k);
                    }
                }
            }
        }
        let missing: Vec<_> = used.difference(&plugin).collect();
        assert!(missing.is_empty(), "claves usadas que no están en es.json: {missing:?}");
        let unused: Vec<_> = plugin.difference(&used).collect();
        assert!(unused.is_empty(), "claves de es.json/plugin que no usa nadie: {unused:?}");
    }

    /// Todos los .rs de una carpeta y sus subcarpetas
    fn rust_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = vec![];
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                out.extend(rust_files(&p));
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
        out
    }

    /// Claves entre comillas simples o dobles tras un prefijo (`data-i18n="`, `T('`…)
    fn after(s: &str, prefix: &str, end: char) -> Vec<String> {
        s.match_indices(prefix).filter_map(|(i, _)| s[i + prefix.len()..].split(end).next().map(String::from)).collect()
    }

    /// Las ventanas de ajustes usan claves que existen, y todas se usan
    #[test]
    fn claves_ventanas() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let pi = keys("es", "pi");
        let mut used = BTreeSet::new();
        for entry in std::fs::read_dir(root.join("pi")).unwrap() {
            let p = entry.unwrap().path();
            if p.extension().is_none_or(|e| e != "html") {
                continue;
            }
            let s = std::fs::read_to_string(&p).unwrap();
            for pre in ["data-i18n=\"", "data-i18n-ph=\"", "data-i18n-title=\""] {
                used.extend(after(&s, pre, '"'));
            }
            used.extend(after(&s, "T('", '\''));
            // Claves guardadas en listas ([valor, 'clave']) que se traducen al pintar
            used.extend(after(&s, "'", '\'').into_iter().filter(|k| pi.contains(k)));
        }
        let missing: Vec<_> = used.difference(&pi).collect();
        assert!(missing.is_empty(), "claves de las ventanas que no están en es.json/pi: {missing:?}");
        let unused: Vec<_> = pi.difference(&used).collect();
        assert!(unused.is_empty(), "claves de es.json/pi que no usa nadie: {unused:?}");
    }

    /// El panel usa claves que existen
    #[test]
    fn claves_panel() {
        let panel = keys("es", "panel");
        // La página entera, tal como la sirve el panel (con su CSS y su código)
        let s = crate::panel::page_source();
        let mut used: BTreeSet<String> = after(&s, "data-t=\"", '"').into_iter().chain(after(&s, "data-tp=\"", '"')).collect();
        for (i, _) in s.match_indices("t.") {
            let before = s[..i].chars().last();
            if before.is_some_and(|c| c.is_alphanumeric() || c == '.' || c == '_') {
                continue;
            }
            let k: String = s[i + 2..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !k.is_empty() {
                used.insert(k);
            }
        }
        let missing: Vec<_> = used.difference(&panel).collect();
        assert!(missing.is_empty(), "claves del panel que no están en es.json/panel: {missing:?}");
    }

}
