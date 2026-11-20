//! Audio de Windows: teclas multimedia, volumen general y por aplicación, y
//! lo que se está reproduciendo.
//!
//! Todo es bloqueante (COM de Windows): llámalo con `tokio::task::spawn_blocking`.
//! En otros sistemas las funciones no hacen nada, para que el resto del plugin
//!
//! - `win`: la parte de Windows (volumen y lo que suena)
//! - `stub`: lo mismo fuera de Windows, sin hacer nada

#[cfg(windows)]
mod win;
#[cfg(windows)]
use win as imp;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
use stub as imp;

pub use imp::*;

/// Teclas multimedia de Windows
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaKey {
    PlayPause,
    Stop,
    Next,
    Prev,
}

/// Lo que suena ahora (Spotify, el navegador, VLC…)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    /// Programa que lo reproduce ("brave.exe")
    pub process: String,
}

/// Qué es una salida de audio (para su dibujo)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(not(windows), allow(dead_code))] // fuera de Windows no hay salidas
pub enum OutputKind {
    #[default]
    Speakers,
    Headphones,
    /// Cascos con micrófono
    Headset,
    /// Monitor o tele (HDMI, DisplayPort)
    Display,
    /// Salida digital (S/PDIF, óptica)
    Digital,
}

/// Una salida de audio de Windows (altavoces, auriculares…)
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub id: String,
    pub name: String,
    pub kind: OutputKind,
    /// Si es la que usa Windows ahora
    pub default: bool,
}

/// Volumen de 0 a 100 y si está silenciado
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    pub volume: u8,
    pub muted: bool,
}

/// Destino del volumen: "general" (o vacío) es el volumen general de Windows;
/// cualquier otra cosa es el nombre del programa, por ejemplo "spotify.exe"
pub fn is_general(target: &str) -> bool {
    let t = target.trim();
    t.is_empty() || t.eq_ignore_ascii_case("general")
}

/// "spotify.exe" → "Spotify"
pub fn display_name(target: &str) -> String {
    if is_general(target) {
        return crate::i18n::t("audio.master").to_string();
    }
    let base = target.trim().trim_end_matches(".exe").trim_end_matches(".EXE");
    let mut c = base.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => target.to_string(),
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub const BROWSERS: [&str; 6] = ["brave.exe", "chrome.exe", "msedge.exe", "firefox.exe", "opera.exe", "vivaldi.exe"];

/// El .exe de un programa según el nombre con el que se presenta al control
/// multimedia de Windows («Spotify.exe», «Chrome», «308046B0AF4A39CB»…)
#[cfg_attr(not(windows), allow(dead_code))]
pub fn exe_of_app(app: &str) -> String {
    let a = app.trim().to_lowercase();
    let known = [
        ("308046b0af4a39cb", "firefox.exe"),
        ("firefox", "firefox.exe"),
        ("msedge", "msedge.exe"),
        ("microsoftedge", "msedge.exe"),
        ("brave", "brave.exe"),
        ("chrome", "chrome.exe"),
        ("opera", "opera.exe"),
        ("vivaldi", "vivaldi.exe"),
        ("spotify", "spotify.exe"),
    ];
    if let Some((_, exe)) = known.iter().find(|(k, _)| a.contains(k)) {
        return exe.to_string();
    }
    if a.ends_with(".exe") {
        return a.rsplit(['\\', '/']).next().unwrap_or(&a).to_string();
    }
    // Apps de la tienda: «Empresa.App_xxx!App» → «app.exe»
    let name = a.split('!').next_back().unwrap_or(&a);
    format!("{name}.exe")
}

/// Saca título y artista del nombre de la ventana de un reproductor conocido
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_title(process: &str, title: &str) -> Option<(String, String)> {
    let title = title.trim();
    if BROWSERS.contains(&process) {
        // "Canción - YouTube Music - Brave" / "Vídeo - YouTube - Brave"
        for mark in [" - YouTube Music", " - YouTube"] {
            if let Some(i) = title.find(mark) {
                let t = title[..i].trim();
                // YouTube pone "(3) Vídeo" cuando hay avisos
                let t = match t.strip_prefix('(').and_then(|r| r.split_once(") ")) {
                    Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest,
                    _ => t,
                };
                return (!t.is_empty()).then(|| (t.to_string(), String::new()));
            }
        }
        return None;
    }
    match process {
        // "Artista - Canción" (en pausa solo pone "Spotify ...")
        "spotify.exe" => {
            let (artist, song) = title.split_once(" - ")?;
            Some((song.trim().to_string(), artist.trim().to_string()))
        }
        "vlc.exe" => {
            let t = title.split(" - VLC").next().unwrap_or(title);
            let t = t.split(" - Reproductor multimedia VLC").next().unwrap_or(t).trim();
            (!t.is_empty() && !t.starts_with("Reproductor") && !t.starts_with("VLC")).then(|| (t.to_string(), String::new()))
        }
        _ => None,
    }
}

/// Cambia el volumen en `delta` puntos (puede ser negativo)
pub fn change_volume(target: &str, delta: i32) -> Option<Level> {
    let now = level(target)?;
    let v = (now.volume as i32 + delta).clamp(0, 100) as u8;
    set_volume(target, v)
}

pub fn toggle_mute(target: &str) -> Option<Level> {
    let now = level(target)?;
    set_mute(target, !now.muted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titulos() {
        assert_eq!(parse_title("brave.exe", "(2) Lofi beats - YouTube - Brave").unwrap().0, "Lofi beats");
        assert_eq!(parse_title("brave.exe", "Canción - YouTube Music - Brave").unwrap().0, "Canción");
        assert!(parse_title("brave.exe", "Google - Brave").is_none());
        assert_eq!(exe_of_app("Spotify.exe"), "spotify.exe");
        assert_eq!(exe_of_app("Chrome"), "chrome.exe");
        assert_eq!(exe_of_app("308046B0AF4A39CB"), "firefox.exe");
        assert_eq!(exe_of_app("C:\\Apps\\vlc.exe"), "vlc.exe");
        assert_eq!(exe_of_app("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "microsoft.zunemusic.exe");
        assert_eq!(parse_title("spotify.exe", "Queen - Bohemian Rhapsody").unwrap(), ("Bohemian Rhapsody".into(), "Queen".into()));
        assert!(parse_title("spotify.exe", "Spotify Premium").is_none());
        assert_eq!(parse_title("vlc.exe", "peli.mkv - Reproductor multimedia VLC").unwrap().0, "peli.mkv");
    }

    #[test]
    fn nombres() {
        assert!(is_general("") && is_general("General") && !is_general("spotify.exe"));
        assert_eq!(display_name("spotify.exe"), "Spotify");
        assert_eq!(display_name("general"), "General");
    }
}
