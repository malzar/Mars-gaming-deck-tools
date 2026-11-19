//! Lo que suena según el control multimedia de Windows (el del panel de
//! volumen, con título y botones): vale para cualquier web del navegador
//! (Crunchyroll, Twitch…), también en pestañas de fondo, y para Spotify.
//!
//! Ese servicio lo da el explorador de Windows y no aguanta que se le
//! pregunte sin parar (llegó a bloquear el escritorio). Así que no se le
//! pregunta cada vez: un hilo propio se apunta a sus avisos (otra sesión,
//! otra canción, play/pausa) y solo entonces lee. El resto de Prisma mira lo
//! último leído. Si el hilo no arranca, o se queda atascado leyendo, se usa
//! la lectura de los nombres de ventana de siempre.
//!
//! Windows mantiene las sesiones en pausa mientras el programa siga abierto
//! (Spotify en pausa durante horas): una pausa solo se muestra un rato
//! (`PAUSED_SHOW`) después de dejar de sonar; luego es «no suena nada».

use std::{
    collections::HashMap,
    sync::{Mutex, Once, mpsc},
    time::{Duration, Instant},
};

use windows::{
    Foundation::TypedEventHandler,
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    },
    core::RuntimeType,
};

use crate::audio::NowPlaying;

/// Juntar los avisos que llegan seguidos antes de leer
const SETTLE: Duration = Duration::from_millis(250);
/// Releer de vez en cuando aunque no avise (por si algún programa no avisa)
const SAFETY: Duration = Duration::from_secs(30);
/// Leyendo más de esto: atascado, se usan las ventanas
const STUCK: Duration = Duration::from_secs(2);
/// Cuánto se sigue mostrando algo en pausa
const PAUSED_SHOW: Duration = Duration::from_secs(30);

struct State {
    /// Sesiones leídas, con la última vez que se las vio sonar (None = el
    /// hilo no funciona)
    sessions: Option<Vec<(NowPlaying, Option<Instant>)>>,
    /// Si está leyendo, desde cuándo
    busy_since: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State { sessions: None, busy_since: None });
static START: Once = Once::new();

/// Lo que dice Windows que suena: `None` si no se puede saber así (y hay que
/// mirar las ventanas)
pub fn current() -> Option<Vec<NowPlaying>> {
    START.call_once(|| {
        let _ = std::thread::Builder::new().name("prisma-media".into()).spawn(|| {
            if let Err(e) = watch() {
                log::warn!("Control multimedia de Windows no disponible ({e}): se usan las ventanas");
            }
            STATE.lock().unwrap().sessions = None;
        });
    });
    let s = STATE.lock().unwrap();
    if s.busy_since.is_some_and(|t| t.elapsed() > STUCK) {
        return None;
    }
    let list = s.sessions.as_ref()?;
    Some(list.iter().filter(|(np, heard)| np.playing || heard.is_some_and(|t| t.elapsed() < PAUSED_SHOW)).map(|(np, _)| np.clone()).collect())
}

fn watch() -> windows::core::Result<()> {
    super::com();
    let manager = Manager::RequestAsync()?.get()?;
    let (tx, rx) = mpsc::channel::<()>();
    manager.SessionsChanged(&poke(&tx))?;
    manager.CurrentSessionChanged(&poke(&tx))?;
    log::info!("Control multimedia de Windows: escuchando sus avisos");
    // Sesiones con sus avisos puestos (para quitarlos al releer)
    let mut hooked: Vec<(Session, i64, i64)> = vec![];
    // Programa y título → última vez que se le vio sonar
    let mut heard: HashMap<(String, String), Instant> = HashMap::new();
    loop {
        STATE.lock().unwrap().busy_since = Some(Instant::now());
        for (s, a, b) in hooked.drain(..) {
            let _ = s.RemoveMediaPropertiesChanged(a);
            let _ = s.RemovePlaybackInfoChanged(b);
        }
        let mut list = vec![];
        if let Ok(sessions) = manager.GetSessions() {
            for s in sessions {
                if let (Ok(a), Ok(b)) = (s.MediaPropertiesChanged(&poke(&tx)), s.PlaybackInfoChanged(&poke(&tx))) {
                    hooked.push((s.clone(), a, b));
                }
                if let Some(np) = read(&s) {
                    let key = (np.process.clone(), np.title.clone());
                    if np.playing {
                        heard.insert(key.clone(), Instant::now());
                    }
                    let last = heard.get(&key).copied();
                    list.push((np, last));
                }
            }
        }
        log::debug!("Control multimedia: {} sesiones", list.len());
        *STATE.lock().unwrap() = State { sessions: Some(list), busy_since: None };
        // Esperar al siguiente aviso (y juntar los que vengan seguidos)
        match rx.recv_timeout(SAFETY) {
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            _ => {
                std::thread::sleep(SETTLE);
                while rx.try_recv().is_ok() {}
            }
        }
    }
}

/// Aviso de Windows: solo despierta al hilo
fn poke<S: RuntimeType + 'static, A: RuntimeType + 'static>(tx: &mpsc::Sender<()>) -> TypedEventHandler<S, A> {
    let tx = tx.clone();
    TypedEventHandler::new(move |_, _| {
        let _ = tx.send(());
        Ok(())
    })
}

fn read(s: &Session) -> Option<NowPlaying> {
    let props = s.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let title = props.Title().ok()?.to_string().trim().to_string();
    if title.is_empty() {
        return None;
    }
    let artist = props.Artist().map(|a| a.to_string().trim().to_string()).unwrap_or_default();
    let playing = s.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).is_ok_and(|st| st == Status::Playing);
    let app = s.SourceAppUserModelId().map(|a| a.to_string()).unwrap_or_default();
    Some(NowPlaying { title, artist, playing, process: crate::audio::exe_of_app(&app) })
}
