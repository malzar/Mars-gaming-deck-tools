//! Reproducir/Pausa, Detener, Siguiente y Anterior.
//!
//! Windows manda las teclas multimedia a su "reproductor actual", que es el
//! último que empezó a sonar, no el de la ventana que estás mirando: con dos
//! YouTube y Spotify la tecla acaba en el que no es, y el que se queda con ella
//! no la suelta. Aquí se elige la ventana y la orden va solo a ella:
//!
//! 1. la ventana que está delante al pulsar, si es de un reproductor;
//! 2. si no, la última ventana de reproductor que estuvo delante;
//! 3. si no, la del programa que está sonando.
//!
//! A YouTube se le manda su atajo de teclado (K, Mayús+N, Mayús+P), que solo
//! afecta a esa pestaña; a Spotify y VLC, la orden directa a su ventana.
//! Cada pulsación deja en el registro a qué ventana fue y por qué.

pub use imp::{send, start_tracking};

#[cfg(windows)]
mod imp {
    use crate::audio::{MediaKey, playing_processes, process_name};
    use std::{
        sync::{
            Once,
            atomic::{AtomicIsize, Ordering},
        },
        time::Duration,
    };
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::{
            Input::KeyboardAndMouse::{
                INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
                SendInput, VIRTUAL_KEY, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK, VK_MEDIA_STOP,
                VK_MENU, VK_SHIFT,
            },
            WindowsAndMessaging::{
                EnumWindows, GW_OWNER, GetForegroundWindow, GetWindow, GetWindowTextLengthW, GetWindowTextW,
                GetWindowThreadProcessId, IsWindow, IsWindowVisible, PostMessageW, SetForegroundWindow, WM_APPCOMMAND,
            },
        },
    };

    fn vk(key: MediaKey) -> VIRTUAL_KEY {
        match key {
            MediaKey::PlayPause => VK_MEDIA_PLAY_PAUSE,
            MediaKey::Stop => VK_MEDIA_STOP,
            MediaKey::Next => VK_MEDIA_NEXT_TRACK,
            MediaKey::Prev => VK_MEDIA_PREV_TRACK,
        }
    }

    /// Número APPCOMMAND_MEDIA_* de cada orden
    fn appcommand(key: MediaKey) -> u32 {
        match key {
            MediaKey::Next => 11,
            MediaKey::Prev => 12,
            MediaKey::Stop => 13,
            MediaKey::PlayPause => 14,
        }
    }

    /// lParam de WM_APPCOMMAND: la orden va en la palabra alta, "viene de una tecla"
    fn appcommand_lparam(key: MediaKey) -> LPARAM {
        LPARAM((appcommand(key) << 16) as isize)
    }

    fn send_input(inputs: &[INPUT]) -> String {
        let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent as usize == inputs.len() {
            "aceptado".into()
        } else {
            format!("Windows solo aceptó {sent} de {} eventos (¿programa delante como administrador?)", inputs.len())
        }
    }

    fn key_input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 },
            },
        }
    }

    /// La tecla multimedia de siempre, como último recurso
    fn press_key(key: MediaKey) -> String {
        let f = KEYEVENTF_EXTENDEDKEY;
        send_input(&[key_input(vk(key), 0, f), key_input(vk(key), 0, f | KEYEVENTF_KEYUP)])
    }

    // ------------------------------------------------------------- ventanas --

    fn pid_of(hwnd: HWND) -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32)) };
        pid
    }

    fn describe(hwnd: HWND) -> String {
        if hwnd.0.is_null() {
            return "ninguna".into();
        }
        process_name(pid_of(hwnd)).unwrap_or_else(|| "?".into())
    }

    /// Ventanas principales (visibles, con título y sin dueño) de unos programas
    fn main_windows_of(processes: &[String]) -> Vec<HWND> {
        unsafe extern "system" fn each(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
            unsafe {
                let list = &mut *(lparam.0 as *mut Vec<HWND>);
                let owned = GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.0.is_null());
                if IsWindowVisible(hwnd).as_bool() && GetWindowTextLengthW(hwnd) > 0 && !owned {
                    list.push(hwnd);
                }
            }
            true.into()
        }
        let mut all: Vec<HWND> = vec![];
        unsafe {
            let _ = EnumWindows(Some(each), LPARAM(&mut all as *mut _ as isize));
        }
        all.into_iter()
            .filter(|h| process_name(pid_of(*h)).is_some_and(|p| processes.contains(&p)))
            .collect()
    }

    fn title_of(hwnd: HWND) -> String {
        let mut buf = [0u16; 512];
        let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    /// La ventana a la que va la orden, solo una: con dos ventanas del
    /// navegador, mandarla a las dos hace que una pare y la otra arranque.
    /// Primero la de un reproductor reconocible (YouTube, Spotify…) del programa
    /// que suena; luego cualquiera reconocible; luego la primera.
    fn best_window(processes: &[String]) -> Option<HWND> {
        let windows = main_windows_of(processes);
        let playing = playing_processes();
        let player = |h: &HWND| {
            let p = process_name(pid_of(*h)).unwrap_or_default();
            crate::audio::parse_title(&p, &title_of(*h)).is_some()
        };
        let sounding = |h: &HWND| process_name(pid_of(*h)).is_some_and(|p| playing.contains(&p));
        windows
            .iter()
            .find(|h| player(h) && sounding(h))
            .or_else(|| windows.iter().find(|h| player(h)))
            .or_else(|| windows.first())
            .copied()
    }

    const PLAYERS: [&str; 9] = [
        "spotify.exe",
        "brave.exe",
        "chrome.exe",
        "msedge.exe",
        "firefox.exe",
        "vlc.exe",
        "opera.exe",
        "vivaldi.exe",
        "foobar2000.exe",
    ];

    /// El programa al que va la orden: el que está sonando; si no suena nada,
    /// los reproductores conocidos que estén abiertos
    fn targets() -> Vec<String> {
        let playing: Vec<String> = playing_processes()
            .into_iter()
            .filter(|p| p != "system" && p != "prisma.exe" && p != "mgdc.exe" && p != "opendeck.exe")
            .collect();
        if !playing.is_empty() {
            return playing;
        }
        PLAYERS.iter().map(|s| s.to_string()).collect()
    }

    // ------------------------------------------------------------ automático --
    //
    // Windows manda las teclas multimedia al "reproductor actual", que es el
    // último que empezó a sonar, no el de la ventana que estás mirando. Por eso
    // con dos YouTube y Spotify la tecla acaba en el que no es, y el que se
    // queda con ella no la suelta. El automático no pasa por ahí: elige él la
    // ventana y le manda la orden solo a ella.

    const BROWSERS: [&str; 6] = ["brave.exe", "chrome.exe", "msedge.exe", "firefox.exe", "opera.exe", "vivaldi.exe"];
    const APPS: [&str; 3] = ["spotify.exe", "vlc.exe", "foobar2000.exe"];

    /// Si la ventana es de un reproductor: el navegador solo cuenta si la
    /// pestaña que se ve es de YouTube; Spotify y VLC siempre (en pausa su
    /// título no dice la canción)
    fn player_of(hwnd: HWND) -> Option<String> {
        if hwnd.0.is_null() || !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return None;
        }
        let p = process_name(pid_of(hwnd))?;
        if APPS.contains(&p.as_str()) {
            return Some(p);
        }
        (BROWSERS.contains(&p.as_str()) && crate::audio::parse_title(&p, &title_of(hwnd)).is_some()).then_some(p)
    }

    /// Última ventana de reproductor que estuvo delante
    static LAST_PLAYER: AtomicIsize = AtomicIsize::new(0);

    /// Vigila qué ventana está delante (cuatro veces por segundo) y apunta la
    /// última que era de un reproductor
    pub fn start_tracking() {
        static STARTED: Once = Once::new();
        STARTED.call_once(|| {
            let _ = std::thread::Builder::new().name("prisma-ventanas".into()).spawn(|| {
                let mut last = 0isize;
                loop {
                    let fg = unsafe { GetForegroundWindow() };
                    if fg.0 as isize != last {
                        last = fg.0 as isize;
                        if player_of(fg).is_some() {
                            LAST_PLAYER.store(last, Ordering::Relaxed);
                        }
                    }
                    std::thread::sleep(Duration::from_millis(250));
                }
            });
        });
    }

    fn press_combo(vks: &[VIRTUAL_KEY]) -> String {
        let mut inputs: Vec<INPUT> = vks.iter().map(|v| key_input(*v, 0, KEYBD_EVENT_FLAGS(0))).collect();
        inputs.extend(vks.iter().rev().map(|v| key_input(*v, 0, KEYEVENTF_KEYUP)));
        send_input(&inputs)
    }

    /// Atajos de YouTube: K reproduce/pausa, Mayús+N siguiente, Mayús+P anterior
    fn youtube_keys(key: MediaKey) -> Vec<VIRTUAL_KEY> {
        match key {
            MediaKey::PlayPause | MediaKey::Stop => vec![VIRTUAL_KEY(0x4B)],
            MediaKey::Next => vec![VK_SHIFT, VIRTUAL_KEY(0x4E)],
            MediaKey::Prev => vec![VK_SHIFT, VIRTUAL_KEY(0x50)],
        }
    }

    /// Manda la orden al reproductor y devuelve qué pasó, para el registro
    pub fn send(key: MediaKey) -> String {
        let front = unsafe { GetForegroundWindow() };
        format!("delante estaba {}: {}", describe(front), auto(key, front))
    }

    fn auto(key: MediaKey, front: HWND) -> String {
        // 1. La ventana que estás mirando, si es de un reproductor
        // 2. Si no, la última de reproductor que estuvo delante
        // 3. Si no, la del programa que suena
        let remembered = HWND(LAST_PLAYER.load(Ordering::Relaxed) as _);
        let (target, why) = if player_of(front).is_some() {
            (front, "la de delante")
        } else if player_of(remembered).is_some() {
            (remembered, "la última que estuvo delante")
        } else {
            match best_window(&targets()).filter(|h| player_of(*h).is_some()) {
                Some(h) => (h, "la del programa que suena"),
                None => return format!("ningún reproductor a la vista; tecla multimedia: {}", press_key(key)),
            }
        };
        let process = player_of(target).unwrap_or_default();
        let name = format!("{} «{}» ({why})", process, title_of(target));
        if BROWSERS.contains(&process.as_str()) {
            let keys = youtube_keys(key);
            if target == front {
                return format!("{name}: atajo de YouTube {}", press_combo(&keys));
            }
            // Se trae un momento delante, se pulsa el atajo y se vuelve
            unsafe {
                send_input(&[key_input(VK_MENU, 0, KEYBD_EVENT_FLAGS(0)), key_input(VK_MENU, 0, KEYEVENTF_KEYUP)]);
                let ok = SetForegroundWindow(target).as_bool();
                std::thread::sleep(Duration::from_millis(60));
                let r = press_combo(&keys);
                std::thread::sleep(Duration::from_millis(60));
                if !front.0.is_null() {
                    let _ = SetForegroundWindow(front);
                }
                LAST_PLAYER.store(target.0 as isize, Ordering::Relaxed);
                format!("{name}: traída delante ({ok}) y atajo de YouTube {r}")
            }
        } else {
            let r = unsafe { PostMessageW(Some(target), WM_APPCOMMAND, WPARAM(target.0 as usize), appcommand_lparam(key)) };
            format!("{name}: orden directa {}", if r.is_ok() { "mandada" } else { "rechazada" })
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::audio::MediaKey;

    pub fn start_tracking() {}

    pub fn send(key: MediaKey) -> String {
        format!("(sin Windows) {key:?}")
    }
}
