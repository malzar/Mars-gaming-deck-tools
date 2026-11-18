//! Icono de cada perfil y color del marco del activo, para la rueda de perfiles.
//!
//! Es solo de Prisma: se configura en el panel (pestaña Perfiles) y no toca nada
//! de OpenDeck. Se guarda en %APPDATA%\prisma\perfiles.json y los iconos en
//! %APPDATA%\prisma\iconos\. Si un perfil no tiene icono, la rueda enseña la
//! imagen de su tecla «Switch Profile», como antes.

use image::{AnimationDecoder, DynamicImage, ImageFormat, RgbaImage, codecs::gif::GifDecoder, imageops::FilterType};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex, RwLock},
};

pub const DEFAULT_BORDER: &str = "#2ecc71";
/// Lado de los fotogramas guardados en memoria
const SIDE: u32 = 128;
const MAX_FRAMES: usize = 150;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DeckLook {
    /// Color del marco del perfil activo ("#rrggbb")
    pub border: Option<String>,
    /// Perfil → fichero del icono (en la carpeta de iconos)
    pub icons: HashMap<String, String>,
    /// Orden de antes de los subperfiles (solo se lee, para no perderlo)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// Jerarquía: qué perfil va dentro de cuál y en qué orden
    pub profiles: Vec<crate::hierarchy::Node>,
}

static LOOKS: LazyLock<RwLock<HashMap<String, DeckLook>>> = LazyLock::new(|| RwLock::new(load()));

/// Fotogramas ya preparados: (imagen SIDE×SIDE, milisegundos)
type Frames = Arc<Vec<(Arc<RgbaImage>, u32)>>;
static FRAMES: LazyLock<Mutex<HashMap<String, Frames>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn base() -> Option<PathBuf> {
    crate::app_dir()
}

fn path() -> Option<PathBuf> {
    Some(base()?.join("perfiles.json"))
}

pub fn icon_dir() -> Option<PathBuf> {
    Some(base()?.join("iconos"))
}

fn load() -> HashMap<String, DeckLook> {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn store(all: &HashMap<String, DeckLook>) -> Result<(), String> {
    let p = path().ok_or(crate::i18n::t("error.no_config_folder"))?;
    std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(all).map_err(|e| e.to_string())?;
    let tmp = p.with_extension("tmp");
    std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &p)).map_err(|e| e.to_string())
}

pub fn look(device: &str) -> DeckLook {
    LOOKS.read().unwrap().get(device).cloned().unwrap_or_default()
}

fn valid_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Color del marco del activo en ese deck
pub fn border(device: &str) -> String {
    look(device).border.filter(|c| valid_color(c)).unwrap_or_else(|| DEFAULT_BORDER.into())
}

pub fn set_border(device: &str, color: &str) -> Result<(), String> {
    if !valid_color(color) {
        return Err(crate::i18n::t("error.color_invalid").into());
    }
    let mut all = LOOKS.write().unwrap();
    all.entry(device.to_string()).or_default().border = Some(color.to_lowercase());
    store(&all)
}

/// Árbol de perfiles de un deck, con los perfiles que existen ahora
pub fn tree(device: &str, existing: &[String]) -> Vec<crate::hierarchy::Node> {
    let l = look(device);
    crate::hierarchy::resolve(&l.profiles, &l.order, existing)
}

/// Guarda el árbol de un deck (el orden de antes ya va dentro)
pub fn save_tree(device: &str, tree: Vec<crate::hierarchy::Node>) -> Result<(), String> {
    // Orden sin huecos dentro de cada padre
    let ids: Vec<String> = tree.iter().map(|n| n.id.clone()).collect();
    let tree = crate::hierarchy::resolve(&tree, &[], &ids);
    let mut all = LOOKS.write().unwrap();
    let l = all.entry(device.to_string()).or_default();
    l.profiles = tree;
    l.order.clear();
    store(&all)
}

/// Un perfil ha cambiado de nombre: su icono y su sitio en la jerarquía (y
/// los subperfiles que cuelgan de él) pasan al nombre nuevo
pub fn rename(device: &str, old: &str, new: &str) -> Result<(), String> {
    let mut all = LOOKS.write().unwrap();
    let Some(l) = all.get_mut(device) else { return Ok(()) };
    if let Some(icon) = l.icons.remove(old) {
        l.icons.insert(new.to_string(), icon);
    }
    for n in l.profiles.iter_mut() {
        if n.id == old {
            n.id = new.to_string();
            n.nombre = new.to_string();
        }
        if n.padre.as_deref() == Some(old) {
            n.padre = Some(new.to_string());
        }
    }
    for o in l.order.iter_mut().filter(|o| *o == old) {
        *o = new.to_string();
    }
    store(&all)
}

/// Fichero del icono de un perfil, si tiene
pub fn icon_file(device: &str, profile: &str) -> Option<PathBuf> {
    let name = look(device).icons.get(profile)?.clone();
    safe_name(&name).then(|| icon_dir().map(|d| d.join(name)))?
}

fn safe_name(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') && !n.starts_with('.')
}

/// Pone (Some, "data:…;base64,…") o quita (None) el icono de un perfil
pub fn set_icon(device: &str, profile: &str, data_url: Option<&str>) -> Result<(), String> {
    let name = match data_url {
        None => None,
        Some(url) => {
            let (bytes, _) = data_url::DataUrl::process(url)
                .ok()
                .and_then(|u| u.decode_to_vec().ok())
                .ok_or(crate::i18n::t("error.file_unreadable"))?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err(crate::i18n::t("error.icon_too_big").into());
            }
            let gif = image::guess_format(&bytes).ok() == Some(ImageFormat::Gif);
            let frames = decode(&bytes)?;
            // Nombre según el contenido: el mismo archivo no se guarda dos veces
            let mut h: u64 = 0xcbf29ce484222325;
            for b in &bytes {
                h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
            }
            let name = format!("{h:016x}.{}", if gif { "gif" } else { "png" });
            let dir = icon_dir().ok_or(crate::i18n::t("error.no_config_folder"))?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            // Lo que no es GIF se guarda ya convertido a PNG
            let out = if gif {
                bytes
            } else {
                let img = image::load_from_memory(&bytes).map_err(|e| crate::i18n::tf("error.image_invalid_detail", &[&e]))?;
                let mut png = Cursor::new(vec![]);
                img.thumbnail(256, 256).write_to(&mut png, ImageFormat::Png).map_err(|e| e.to_string())?;
                png.into_inner()
            };
            std::fs::write(dir.join(&name), out).map_err(|e| e.to_string())?;
            FRAMES.lock().unwrap().insert(name.clone(), Arc::new(frames));
            Some(name)
        }
    };
    let mut all = LOOKS.write().unwrap();
    let old = {
        let l = all.entry(device.to_string()).or_default();
        match name {
            Some(n) => l.icons.insert(profile.to_string(), n),
            None => l.icons.remove(profile),
        }
    };
    store(&all)?;
    // Borra el fichero viejo si ya no lo usa ningún perfil
    if let Some(old) = old
        && safe_name(&old)
        && !all.values().any(|l| l.icons.values().any(|n| *n == old))
        && let Some(d) = icon_dir()
    {
        let _ = std::fs::remove_file(d.join(&old));
        FRAMES.lock().unwrap().remove(&old);
    }
    Ok(())
}

/// Fotogramas cuadrados de SIDE×SIDE (uno solo si no es GIF)
fn decode(bytes: &[u8]) -> Result<Vec<(Arc<RgbaImage>, u32)>, String> {
    let square = |img: DynamicImage| Arc::new(img.resize_to_fill(SIDE, SIDE, FilterType::Triangle).to_rgba8());
    if image::guess_format(bytes).ok() == Some(ImageFormat::Gif) {
        let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(|e| crate::i18n::tf("error.gif_invalid", &[&e]))?;
        let mut out = vec![];
        for f in decoder.into_frames().take(MAX_FRAMES).filter_map(|f| f.ok()) {
            let (num, den) = f.delay().numer_denom_ms();
            let ms = num.checked_div(den).map_or(100, |ms| ms.max(60));
            out.push((square(DynamicImage::ImageRgba8(f.into_buffer())), ms));
        }
        if out.is_empty() { Err(crate::i18n::t("error.gif_no_frames").into()) } else { Ok(out) }
    } else {
        let img = image::load_from_memory(bytes).map_err(|e| crate::i18n::tf("error.image_invalid_detail", &[&e]))?;
        Ok(vec![(square(img), 0)])
    }
}

fn frames(device: &str, profile: &str) -> Option<Frames> {
    let name = look(device).icons.get(profile)?.clone();
    if let Some(f) = FRAMES.lock().unwrap().get(&name).cloned() {
        return Some(f);
    }
    let bytes = std::fs::read(icon_file(device, profile)?).ok()?;
    let f = Arc::new(decode(&bytes).ok()?);
    FRAMES.lock().unwrap().insert(name, f.clone());
    Some(f)
}

/// Si el icono de ese perfil se mueve (GIF con más de un fotograma)
pub fn animated(device: &str, profile: &str) -> bool {
    frames(device, profile).is_some_and(|f| f.len() > 1)
}

/// El fotograma del icono que toca en el segundo `t`
pub fn frame(device: &str, profile: &str, t: f32) -> Option<Arc<RgbaImage>> {
    let frames = frames(device, profile)?;
    let total: u32 = frames.iter().map(|(_, ms)| ms).sum();
    let mut at = ((t * 1000.0) as u32) % total.max(1);
    for (img, ms) in frames.iter() {
        if at < *ms {
            return Some(img.clone());
        }
        at -= ms;
    }
    Some(frames[0].0.clone())
}

/// Pega `pic` en el recuadro de la pantalla de la rueda que ya ha dibujado
/// OpenDeck (su lienzo es de 200×100; el recuadro está en 10,14 y mide 72),
/// por dentro del marco y con las esquinas redondeadas
pub fn paste(base: &DynamicImage, pic: &RgbaImage) -> RgbaImage {
    let mut out = base.to_rgba8();
    let (w, h) = (out.width() as f32, out.height() as f32);
    let k = (w / 200.0).min(h / 100.0);
    let (ox, oy) = ((w - 200.0 * k) / 2.0, (h - 100.0 * k) / 2.0);
    let inset = 3.0;
    let x0 = ox + (10.0 + inset) * k;
    let y0 = oy + (14.0 + inset) * k;
    let side = ((72.0 - 2.0 * inset) * k).round().max(1.0) as u32;
    let r = 6.0 * k;
    let pic = image::imageops::resize(pic, side, side, FilterType::Triangle);
    for (px, py, p) in pic.enumerate_pixels() {
        // Esquinas redondeadas
        let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
        let cx = fx.clamp(r, side as f32 - r);
        let cy = fy.clamp(r, side as f32 - r);
        let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
        let edge = (r + 0.5 - d).clamp(0.0, 1.0);
        let a = p[3] as f32 / 255.0 * edge;
        if a <= 0.0 {
            continue;
        }
        let (x, y) = ((x0 + px as f32) as u32, (y0 + py as f32) as u32);
        if x >= out.width() || y >= out.height() {
            continue;
        }
        let q = out.get_pixel_mut(x, y);
        for c in 0..3 {
            q[c] = (p[c] as f32 * a + q[c] as f32 * (1.0 - a)).round() as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colores() {
        assert!(valid_color("#ff9f1a"));
        assert!(!valid_color("ff9f1a"));
        assert!(!valid_color("#ff9f1g"));
        assert!(safe_name("0123abcd.gif"));
        assert!(!safe_name("../x.png"));
    }

    #[test]
    fn gif_animado() {
        use image::{Delay, Frame, RgbaImage, codecs::gif::GifEncoder};
        let mut bytes = vec![];
        {
            let mut enc = GifEncoder::new(&mut bytes);
            for c in [255u8, 0] {
                let img = RgbaImage::from_pixel(20, 20, image::Rgba([c, 0, 0, 255]));
                enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(100, 1))).unwrap();
            }
        }
        let f = decode(&bytes).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].1, 100);
    }
}

/// Pantallas de la rueda con un GIF de verdad:
/// XDG_CONFIG_HOME=tmp PRISMA_GIF=x.gif PRISMA_SAMPLES_OUT=carpeta cargo test muestras_iconos -- --ignored
#[cfg(test)]
#[test]
#[ignore]
fn muestras_iconos() {
    let out = std::env::var("PRISMA_SAMPLES_OUT").unwrap();
    let gif = std::fs::read(std::env::var("PRISMA_GIF").unwrap()).unwrap();
    let url = format!("data:image/gif;base64,{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, gif));
    set_icon("deck", "aviones", Some(&url)).unwrap();
    set_border("deck", "#ff9f1a").unwrap();
    std::fs::write(format!("{out}/base.svg"), crate::profiles::render_slot(&crate::profiles::SlotView { title: "Perfiles", name: Some("aviones"), badge: "▸ 2 subperfiles", at: 2, total: 4, active: true, direct: false, border: &border("deck") })).unwrap();
    assert!(animated("deck", "aviones"));
    for (i, t) in [0.0f32, 0.5, 1.0].iter().enumerate() {
        let f = frame("deck", "aviones", *t).unwrap();
        // Lo que manda OpenDeck: el SVG de la rueda pasado a imagen (aquí, un fondo plano)
        let base = match std::env::var("PRISMA_BASE") {
            Ok(p) => image::open(p).unwrap(),
            Err(_) => DynamicImage::ImageRgba8(RgbaImage::from_pixel(200, 100, image::Rgba([0, 0, 0, 255]))),
        };
        paste(&base, &f).save(format!("{out}/rueda-{i}.png")).unwrap();
    }
    set_icon("deck", "aviones", None).unwrap();
    assert!(icon_file("deck", "aviones").is_none());
}

#[cfg(test)]
#[test]
fn orden() {
    let order = ["coches".to_string(), "Default".to_string()];
    let mut p: Vec<String> = ["Default", "aviones", "coches", "test1"].iter().map(|s| s.to_string()).collect();
    p.sort_by_key(|x| order.iter().position(|o| o == x).unwrap_or(usize::MAX));
    assert_eq!(p, ["coches", "Default", "aviones", "test1"]);
}

/// El perfiles.json de ejemplo: XDG_CONFIG_HOME=carpeta cargo test ejemplo_json -- --ignored
#[cfg(test)]
#[test]
#[ignore]
fn ejemplo_json() {
    use crate::hierarchy::set_parent;
    let existing: Vec<String> = ["Default", "aviones", "F-18", "Cessna", "coches"].iter().map(|s| s.to_string()).collect();
    let mut t = tree("99-C771C7782E1B", &existing);
    set_parent(&mut t, "F-18", Some("aviones")).unwrap();
    set_parent(&mut t, "Cessna", Some("aviones")).unwrap();
    save_tree("99-C771C7782E1B", t).unwrap();
    set_border("99-C771C7782E1B", "#2ecc71").unwrap();
}
