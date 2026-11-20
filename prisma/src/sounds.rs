//! Sonidos de aviso (temporizador y Pomodoro).
//!
//! - Cinco de serie, hechos para Prisma: `assets/sounds/<nombre>.wav`.
//! - El sonido de aviso de Windows ("system", o cualquier id que no sea un fichero).
//! - Uno propio: un WAV que se elige en la ventana de ajustes y se guarda en
//!   %APPDATA%\prisma\sounds.

use std::path::PathBuf;

/// Los de serie, en el orden en que salen en la lista
pub const BUILTIN: [&str; 13] = ["campana", "digital", "marimba", "alarma", "suave", "cucaracha", "trombon", "boing", "lluvia", "sarten", "viento", "trueno", "ventisca"];
/// Los propios van como "custom:<fichero>"
const CUSTOM: &str = "custom:";
/// Tamaño máximo de un sonido propio
const MAX_BYTES: usize = 2 * 1024 * 1024;

fn custom_dir() -> Option<PathBuf> {
    Some(crate::app_dir()?.join("sounds"))
}

/// Fichero de un sonido (None = el de Windows o uno que no existe)
pub fn path(id: &str) -> Option<PathBuf> {
    if let Some(file) = id.strip_prefix(CUSTOM) {
        // Solo nombres que haya puesto Prisma: letras, números y punto
        if file.is_empty() || !file.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') || file.starts_with('.') {
            return None;
        }
        return custom_dir().map(|d| d.join(file)).filter(|p| p.is_file());
    }
    if BUILTIN.contains(&id) {
        let exe = std::env::current_exe().ok()?;
        return Some(exe.parent()?.join("assets").join("sounds").join(format!("{id}.wav"))).filter(|p| p.is_file());
    }
    None
}

/// Guarda un WAV propio (data URL) y devuelve su id
pub fn save_custom(data_url: &str) -> Result<String, String> {
    let (bytes, _) = data_url::DataUrl::process(data_url)
        .ok()
        .and_then(|u| u.decode_to_vec().ok())
        .ok_or(crate::i18n::t("error.file_unreadable"))?;
    if bytes.len() > MAX_BYTES {
        return Err(crate::i18n::t("error.sound_too_big").into());
    }
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(crate::i18n::t("error.sound_not_wav").into());
    }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    let name = format!("{:016x}.wav", h.finish());
    let dir = custom_dir().ok_or(crate::i18n::t("error.no_config_folder"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(&name), &bytes).map_err(|e| e.to_string())?;
    Ok(format!("{CUSTOM}{name}"))
}

/// Suena (sin esperar a que acabe). Si el fichero no está, suena el de Windows.
pub fn play(id: &str) {
    let file = path(id);
    log::info!("Sonido: {id} ({})", file.as_ref().map_or("Windows".into(), |p| p.display().to_string()));
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::{
            Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT},
            System::Diagnostics::Debug::MessageBeep,
            UI::WindowsAndMessaging::MB_ICONASTERISK,
        };
        match file {
            Some(p) => {
                let wide = windows::core::HSTRING::from(p.as_os_str());
                // SAFETY: la ruta vive hasta que vuelve la llamada; con SND_ASYNC Windows hace su copia
                let _ = unsafe { PlaySoundW(&wide, None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT) };
            }
            // SAFETY: llamada sin punteros; solo pide a Windows que suene
            None => {
                let _ = unsafe { MessageBeep(MB_ICONASTERISK) };
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = file;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sonidos() {
        // Los de serie están en assets/sounds y son WAV
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sounds");
        for s in BUILTIN {
            let b = std::fs::read(dir.join(format!("{s}.wav"))).unwrap();
            assert!(&b[0..4] == b"RIFF" && &b[8..12] == b"WAVE", "{s}");
        }
        // Nombres raros de sonidos propios no salen de su carpeta
        assert!(path("custom:../../x.wav").is_none());
        assert!(path("custom:").is_none());
        assert!(path("system").is_none());
        // Solo WAV
        assert!(save_custom("data:audio/mp3;base64,SUQzBAAAAAAAI1RTU0UAAAAPAAAD").is_err());
    }
}
