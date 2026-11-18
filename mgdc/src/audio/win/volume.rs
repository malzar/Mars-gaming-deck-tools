//! Volumen: los altavoces, la sesión de sonido de cada programa y sus nombres

use std::time::{Duration, Instant};

use windows::{
    Win32::{
        Foundation::CloseHandle,
        Media::Audio::{
            Endpoints::IAudioEndpointVolume, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
            eConsole, eRender,
        },
        System::{
            Com::{CLSCTX_ALL, CoCreateInstance},
            Threading::{OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW},
        },
    },
    core::{Interface, PWSTR},
};

use super::{ENDPOINT, KEEP, NAMES, SESSIONS, com};
use crate::audio::{Level, is_general};

fn endpoint() -> windows::core::Result<IAudioEndpointVolume> {
    if let Some((ep, at)) = ENDPOINT.with_borrow(|c| c.clone())
        && at.elapsed() < KEEP
    {
        return Ok(ep);
    }
    let r = open_endpoint();
    if let Ok(ep) = &r {
        ENDPOINT.set(Some((ep.clone(), Instant::now())));
    }
    r
}

fn open_endpoint() -> windows::core::Result<IAudioEndpointVolume> {
    com();
    let r = unsafe {
        CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .and_then(|e| e.GetDefaultAudioEndpoint(eRender, eConsole))
            .and_then(|d| d.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None))
    };
    if let Err(e) = &r {
        log::warn!("Audio: no se pudo abrir el volumen general: {e}");
    }
    r
}

pub(super) fn session_manager() -> windows::core::Result<IAudioSessionManager2> {
    if let Some((m, at)) = SESSIONS.with_borrow(|c| c.clone())
        && at.elapsed() < KEEP
    {
        return Ok(m);
    }
    let r = open_session_manager();
    if let Ok(m) = &r {
        SESSIONS.set(Some((m.clone(), Instant::now())));
    }
    r
}

fn open_session_manager() -> windows::core::Result<IAudioSessionManager2> {
    com();
    let r = unsafe {
        CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .and_then(|e| e.GetDefaultAudioEndpoint(eRender, eConsole))
            .and_then(|d| d.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None))
    };
    if let Err(e) = &r {
        log::warn!("Audio: no se pudo abrir el volumen de los programas: {e}");
    }
    r
}

/// Nombre del ejecutable de un proceso ("spotify.exe"), en minúsculas
pub fn process_name(pid: u32) -> Option<String> {
    if pid == 0 {
        return Some("system".to_string());
    }
    // Los nombres se guardan un rato: abrir cada proceso en cada consulta es lento
    let cached = NAMES.with_borrow_mut(|(names, at)| {
        if at.is_none_or(|t| t.elapsed() > Duration::from_secs(30)) {
            names.clear();
            *at = Some(Instant::now());
        }
        names.get(&pid).cloned()
    });
    if cached.is_some() {
        return cached;
    }
    let name = open_process_name(pid)?;
    NAMES.with_borrow_mut(|(names, _)| names.insert(pid, name.clone()));
    Some(name)
}

fn open_process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(handle);
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(|s| s.to_lowercase())
    }
}

/// Sesiones de audio de un programa (un programa puede tener varias)
fn sessions_of(target: &str) -> Vec<ISimpleAudioVolume> {
    let target = target.trim().to_lowercase();
    let mut out = vec![];
    let Ok(manager) = session_manager() else { return out };
    unsafe {
        let Ok(list) = manager.GetSessionEnumerator() else { return out };
        let count = list.GetCount().unwrap_or(0);
        for i in 0..count {
            let Ok(control) = list.GetSession(i) else { continue };
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
            let pid = control2.GetProcessId().unwrap_or(0);
            if process_name(pid).as_deref() == Some(target.as_str())
                && let Ok(vol) = control.cast::<ISimpleAudioVolume>()
            {
                out.push(vol);
            }
        }
    }
    out
}

/// Programas que tienen sonido abierto ahora mismo (sin repetir, sin "system")
pub fn apps() -> Vec<String> {
    let mut names = vec![];
    let Ok(manager) = session_manager() else { return names };
    unsafe {
        let Ok(list) = manager.GetSessionEnumerator() else { return names };
        let count = list.GetCount().unwrap_or(0);
        for i in 0..count {
            let Ok(control) = list.GetSession(i) else { continue };
            let Ok(control2) = control.cast::<IAudioSessionControl2>() else { continue };
            if let Some(name) = process_name(control2.GetProcessId().unwrap_or(0))
                && name != "system"
                && !names.contains(&name)
            {
                names.push(name);
            }
        }
    }
    names.sort();
    names
}

pub fn level(target: &str) -> Option<Level> {
    unsafe {
        if is_general(target) {
            let ep = endpoint().ok()?;
            let v = ep.GetMasterVolumeLevelScalar().ok()?;
            let m = ep.GetMute().ok()?.as_bool();
            Some(Level { volume: (v * 100.0).round() as u8, muted: m })
        } else {
            let s = sessions_of(target);
            let first = s.first()?;
            let v = first.GetMasterVolume().ok()?;
            let m = first.GetMute().ok()?.as_bool();
            Some(Level { volume: (v * 100.0).round() as u8, muted: m })
        }
    }
}

pub fn set_volume(target: &str, volume: u8) -> Option<Level> {
    let v = volume.min(100) as f32 / 100.0;
    unsafe {
        if is_general(target) {
            let ep = endpoint().ok()?;
            ep.SetMasterVolumeLevelScalar(v, std::ptr::null()).ok()?;
            // Subir el volumen quita el silencio, como en Windows
            let _ = ep.SetMute(false, std::ptr::null());
        } else {
            let s = sessions_of(target);
            if s.is_empty() {
                return None;
            }
            for vol in &s {
                let _ = vol.SetMasterVolume(v, std::ptr::null());
                let _ = vol.SetMute(false, std::ptr::null());
            }
        }
    }
    level(target)
}

pub fn set_mute(target: &str, muted: bool) -> Option<Level> {
    unsafe {
        if is_general(target) {
            endpoint().ok()?.SetMute(muted, std::ptr::null()).ok()?;
        } else {
            let s = sessions_of(target);
            if s.is_empty() {
                return None;
            }
            for vol in &s {
                let _ = vol.SetMute(muted, std::ptr::null());
            }
        }
    }
    level(target)
}
