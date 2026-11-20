//! Lo que suena: qué programas tienen sonido, sus ventanas y el volumen real

use std::{
    cell::RefCell,
    collections::HashMap,
    time::{Duration, Instant},
};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM},
        Media::Audio::{AudioSessionStateActive, Endpoints::IAudioMeterInformation, IAudioSessionControl2, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender},
        System::Com::{CLSCTX_ALL, CoCreateInstance},
        UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible},
    },
    core::Interface,
};

use super::{com, volume::{process_name, session_manager}};
use crate::audio::{NowPlaying, parse_title};

// ------------------------------------------------------------ lo que suena --
//
// Primero lo que diga el control multimedia de Windows (ver
// `media_session`: solo lee cuando Windows avisa de un cambio). Si eso no
// funciona, del nombre de las ventanas (Brave pone "Vídeo - YouTube -
// Brave", Spotify "Artista - Canción").

/// Programas con sonido saliendo ahora mismo
pub fn playing_processes() -> Vec<String> {
    let mut out = vec![];
    let Ok(manager) = session_manager() else { return out };
    unsafe {
        let Ok(list) = manager.GetSessionEnumerator() else { return out };
        for i in 0..list.GetCount().unwrap_or(0) {
            let Ok(control) = list.GetSession(i) else { continue };
            if control.GetState().ok() != Some(AudioSessionStateActive) {
                continue;
            }
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
            if let Some(name) = process_name(control2.GetProcessId().unwrap_or(0)) {
                out.push(name);
            }
        }
    }
    out
}

/// Ventanas visibles con título: (programa, título)
fn window_titles() -> Vec<(String, String)> {
    unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
        unsafe {
            let list = &mut *(lparam.0 as *mut Vec<(u32, String)>);
            if IsWindowVisible(hwnd).as_bool() {
                let mut buf = [0u16; 512];
                let n = GetWindowTextW(hwnd, &mut buf);
                if n > 0 {
                    let mut pid = 0u32;
                    GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
                    list.push((pid, String::from_utf16_lossy(&buf[..n as usize])));
                }
            }
        }
        true.into()
    }
    let mut raw: Vec<(u32, String)> = vec![];
    unsafe {
        let _ = EnumWindows(Some(each), LPARAM(&mut raw as *mut _ as isize));
    }
    raw.into_iter().filter_map(|(pid, t)| Some((process_name(pid)?, t))).collect()
}

/// Lo fuerte que suena ahora un programa (0 a 1); vacío = todo el sonido
pub fn peak(process: &str) -> Option<f32> {
    unsafe {
        if process.is_empty() {
            com();
            let meter: IAudioMeterInformation = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .and_then(|e| e.GetDefaultAudioEndpoint(eRender, eConsole))
                .and_then(|d| d.Activate::<IAudioMeterInformation>(CLSCTX_ALL, None))
                .ok()?;
            return meter.GetPeakValue().ok();
        }
        let manager = session_manager().ok()?;
        let list = manager.GetSessionEnumerator().ok()?;
        let mut best: Option<f32> = None;
        for i in 0..list.GetCount().unwrap_or(0) {
            let Ok(control) = list.GetSession(i) else { continue };
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
            if process_name(control2.GetProcessId().unwrap_or(0)).as_deref() != Some(process) {
                continue;
            }
            if let Ok(m) = control.cast::<IAudioMeterInformation>()
                && let Ok(v) = m.GetPeakValue()
            {
                best = Some(best.map_or(v, |b: f32| b.max(v)));
            }
        }
        best
    }
}

thread_local! {
    /// Última vez que se oyó algo de cada programa
    static HEARD: RefCell<HashMap<String, Instant>> = RefCell::new(HashMap::new());
}

/// Si un programa ha sonado hace nada. Windows deja la sesión de sonido
/// "activa" unos segundos después de pausar (el navegador sobre todo), así
/// que se mira el nivel real: en pausa cae a cero al momento.
fn audible(process: &str) -> bool {
    let loud = peak(process).is_some_and(|v| v > 0.001);
    HEARD.with_borrow_mut(|h| {
        if loud {
            h.insert(process.to_string(), Instant::now());
        }
        h.get(process).is_some_and(|t| t.elapsed() < Duration::from_millis(1500))
    })
}

pub fn now_playing() -> Option<NowPlaying> {
    // Si Windows lo sabe, manda (aunque diga que no suena nada)
    if let Some(list) = super::media_session::current() {
        return list.iter().find(|n| n.playing).or(list.first()).cloned();
    }
    let active = playing_processes();
    let mut found: Option<NowPlaying> = None;
    let mut heard: HashMap<String, bool> = HashMap::new();
    for (process, title) in window_titles() {
        let Some((title, artist)) = parse_title(&process, &title) else { continue };
        let playing = active.contains(&process)
            && *heard.entry(process.clone()).or_insert_with(|| audible(&process));
        // Primero lo que está sonando; si nada suena, lo primero que se encuentre
        if playing {
            return Some(NowPlaying { title, artist, playing: true, process });
        }
        if found.is_none() {
            found = Some(NowPlaying { title, artist, playing: false, process });
        }
    }
    found
}
