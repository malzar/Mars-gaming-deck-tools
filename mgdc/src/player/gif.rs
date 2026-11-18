//! El GIF propio de la barra de la tecla Reproducir/Pausa: se guarda al
//! subirlo y se reparte en fotogramas ya recortados a la barra

use std::{
    collections::HashMap,
    io::Cursor,
    path::PathBuf,
    sync::{Arc, LazyLock, Mutex},
};

use base64::Engine;
use image::{AnimationDecoder, DynamicImage, ImageFormat, codecs::gif::GifDecoder, imageops::FilterType};

/// Carpeta donde se guardan los GIF de las teclas
fn gif_dir() -> Option<PathBuf> {
    Some(crate::app_dir()?.join("keys"))
}

/// Guarda un GIF que llega de la ventana de ajustes ("data:image/gif;base64,…")
/// y devuelve su nombre, que es lo que se guarda en los ajustes de la tecla
pub fn save_gif(data_url: &str) -> Result<String, String> {
    let b64 = data_url.split_once(',').map(|(_, b)| b).unwrap_or(data_url);
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| crate::i18n::t("error.file_unreadable"))?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(crate::i18n::t("error.gif_too_big").into());
    }
    let frames = decode(&bytes)?;
    // Nombre según el contenido: el mismo GIF no se guarda dos veces
    let mut h: u64 = 0xcbf29ce484222325;
    for b in &bytes {
        h = (h ^ *b as u64).wrapping_mul(0x100000001b3);
    }
    let id = format!("{h:016x}");
    let dir = gif_dir().ok_or(crate::i18n::t("error.no_config_folder"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(format!("{id}.gif")), &bytes).map_err(|e| e.to_string())?;
    GIFS.lock().unwrap().insert(id.clone(), Arc::new(frames));
    Ok(id)
}

type Frames = Arc<Vec<(String, u32)>>;
static GIFS: LazyLock<Mutex<HashMap<String, Frames>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Fotogramas del GIF ya recortados a la barra (144×16) y en PNG
fn decode(bytes: &[u8]) -> Result<Vec<(String, u32)>, String> {
    let decoder = GifDecoder::new(Cursor::new(bytes)).map_err(|e| crate::i18n::tf("error.gif_invalid", &[&e]))?;
    let mut out = vec![];
    for f in decoder.into_frames().take(120).filter_map(|f| f.ok()) {
        let (num, den) = f.delay().numer_denom_ms();
        let ms = num.checked_div(den).map_or(100, |ms| ms.max(40));
        let img = DynamicImage::ImageRgba8(f.into_buffer()).resize_to_fill(144, 16, FilterType::Triangle);
        let mut png = Cursor::new(vec![]);
        img.write_to(&mut png, ImageFormat::Png).map_err(|e| e.to_string())?;
        out.push((base64::engine::general_purpose::STANDARD.encode(png.into_inner()), ms));
    }
    if out.is_empty() { Err(crate::i18n::t("error.gif_no_frames").into()) } else { Ok(out) }
}

/// Fotograma del GIF que toca en el segundo `t`
pub fn gif_frame(id: &str, t: f32) -> Option<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let cached = GIFS.lock().unwrap().get(id).cloned();
    let frames = match cached {
        Some(f) => f,
        None => {
            let bytes = std::fs::read(gif_dir()?.join(format!("{id}.gif"))).ok()?;
            let f = Arc::new(decode(&bytes).ok()?);
            GIFS.lock().unwrap().insert(id.to_string(), f.clone());
            f
        }
    };
    let total: u32 = frames.iter().map(|(_, ms)| ms).sum();
    let mut at = ((t * 1000.0) as u32) % total.max(1);
    for (png, ms) in frames.iter() {
        if at < *ms {
            return Some(png.clone());
        }
        at -= ms;
    }
    frames.first().map(|(p, _)| p.clone())
}
