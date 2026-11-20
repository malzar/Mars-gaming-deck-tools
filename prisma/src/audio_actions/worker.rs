//! Hilos que hablan con Windows (volumen, lo que suena, teclas multimedia).
//! Las llamadas a Windows bloquean, así que van por hilos propios y nunca
//! paran al resto del plugin.

use std::{sync::LazyLock, time::Duration};

pub(super) type Job = Box<dyn FnOnce() + Send>;

pub(super) type Worker = LazyLock<std::sync::Mutex<std::sync::mpsc::Sender<Job>>>;

fn worker(name: &'static str) -> std::sync::Mutex<std::sync::mpsc::Sender<Job>> {
    let (tx, rx) = std::sync::mpsc::channel::<Job>();
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                // Si una llamada a Windows revienta, el hilo sigue vivo
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).is_err() {
                    log::error!("Audio: fallo dentro de {name}");
                }
            }
        })
        .expect("hilo de audio");
    std::sync::Mutex::new(tx)
}

/// Volumen y teclas multimedia van por un solo hilo: así los objetos de
/// Windows se abren una vez y se reutilizan, y los giros se aplican en orden.
pub(super) static AUDIO_THREAD: Worker = LazyLock::new(|| worker("prisma-audio"));
/// "Lo que suena" va aparte: si Windows tarda en contestar, el volumen no espera
pub(super) static MEDIA_THREAD: Worker = LazyLock::new(|| worker("prisma-media"));
/// Reproducir/Pausa y compañía, también aparte: la pulsación va siempre al momento
pub(super) static KEY_THREAD: Worker = LazyLock::new(|| worker("prisma-teclas"));
pub(super) static MEDIA_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Llama a Windows en el hilo de audio, como mucho 3 segundos
pub(super) async fn run_blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    run_on(&AUDIO_THREAD, f).await
}

pub(super) async fn run_on<T: Send + 'static>(thread: &Worker, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let job: Job = Box::new(move || {
        let _ = tx.send(f());
    });
    if thread.lock().ok()?.send(job).is_err() {
        log::error!("Audio: el hilo de audio no está");
        return None;
    }
    match tokio::time::timeout(Duration::from_secs(3), rx).await {
        Ok(Ok(v)) => Some(v),
        Ok(Err(_)) => {
            log::error!("Audio: la consulta a Windows ha fallado");
            None
        }
        Err(_) => {
            log::error!("Audio: Windows no ha contestado en 3 s");
            None
        }
    }
}
