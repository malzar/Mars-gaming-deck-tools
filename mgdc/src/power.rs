//! Brillo en tiempo real, apagado por inactividad y despertar al pulsar.
//!
//! "Apagar la pantalla" se hace poniendo el brillo a 0: así no hay que volver a
//! enviar las imágenes al despertar.

use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

use chrono::Local;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    DEVICES,
    schedule::target_brightness,
    settings::{self, CHANGED},
};

pub struct Runtime {
    pub last_activity: Instant,
    pub asleep: bool,
    /// Último brillo enviado al deck (None = hay que reenviarlo)
    pub applied: Option<u8>,
    /// Ignorar la siguiente "suelta" (de la pulsación que despertó la pantalla)
    pub swallow_release: bool,
    /// Mientras se identifica el deck, el planificador no toca el brillo
    pub paused: bool,
}

impl Runtime {
    fn new() -> Self {
        Self {
            last_activity: Instant::now(),
            asleep: false,
            applied: None,
            swallow_release: false,
            paused: false,
        }
    }
}

pub static RUNTIME: LazyLock<Mutex<HashMap<String, Runtime>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

async fn set_device_brightness(id: &str, value: u8) {
    if let Some(device) = DEVICES.read().await.get(id) {
        if let Err(e) = device.set_brightness(value.min(100)).await {
            log::warn!("No se pudo poner el brillo de {id}: {e}");
        }
    }
}

/// Brillo inicial al conectar un deck
pub async fn initial_brightness(id: &str) -> u8 {
    let s = settings::for_device(id).await;
    let target = target_brightness(&s, &Local::now());
    let mut rt = RUNTIME.lock().await;
    let r = rt.entry(id.to_string()).or_insert_with(Runtime::new);
    *r = Runtime::new();
    r.applied = Some(target);
    target
}

/// Se llama con cada entrada del deck. Devuelve true si hay que ignorarla
/// (la pulsación que solo sirve para despertar la pantalla).
pub async fn on_input(id: &str, is_release: bool) -> bool {
    let s = settings::for_device(id).await;
    let mut rt = RUNTIME.lock().await;
    let r = rt.entry(id.to_string()).or_insert_with(Runtime::new);
    r.last_activity = Instant::now();

    if r.asleep {
        r.asleep = false;
        r.applied = None;
        r.swallow_release = s.swallow_wake_press && !is_release;
        drop(rt);
        CHANGED.notify_one();
        return s.swallow_wake_press;
    }

    if is_release && r.swallow_release {
        r.swallow_release = false;
        return true;
    }

    false
}

/// Pone un brillo al momento (el panel lo usa al mover el deslizador)
pub async fn preview_brightness(id: &str, value: u8) {
    {
        let mut rt = RUNTIME.lock().await;
        let r = rt.entry(id.to_string()).or_insert_with(Runtime::new);
        r.asleep = false;
        r.last_activity = Instant::now();
        r.applied = Some(value);
    }
    set_device_brightness(id, value).await;
}

/// Hace parpadear el deck para saber cuál es
pub async fn identify(id: &str) {
    {
        let mut rt = RUNTIME.lock().await;
        let r = rt.entry(id.to_string()).or_insert_with(Runtime::new);
        r.paused = true;
        r.asleep = false;
        r.last_activity = Instant::now();
    }
    for _ in 0..3 {
        set_device_brightness(id, 100).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
        set_device_brightness(id, 5).await;
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    {
        let mut rt = RUNTIME.lock().await;
        if let Some(r) = rt.get_mut(id) {
            r.paused = false;
            r.applied = None;
        }
    }
    CHANGED.notify_one();
}

/// Revisa cada 20 s (o al momento si algo cambia) el brillo y el apagado
pub async fn scheduler_task(token: CancellationToken) {
    loop {
        tick().await;

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(20)) => {}
            _ = CHANGED.notified() => {}
            _ = token.cancelled() => break,
        }
    }
}

async fn tick() {
    let now = Local::now();
    let ids: Vec<String> = DEVICES.read().await.keys().cloned().collect();

    for id in ids {
        let s = settings::for_device(&id).await;
        let target = target_brightness(&s, &now);

        let action = {
            let mut rt = RUNTIME.lock().await;
            let r = rt.entry(id.clone()).or_insert_with(Runtime::new);

            if r.paused {
                None
            } else if s.never_sleep && r.asleep {
                // Se ha desactivado el apagado: despertar
                r.asleep = false;
                r.applied = Some(target);
                Some(target)
            } else if !s.never_sleep
                && !r.asleep
                && r.last_activity.elapsed()
                    >= Duration::from_secs(s.sleep_minutes.max(1) as u64 * 60)
            {
                log::info!("Apagando la pantalla de {id} por inactividad");
                r.asleep = true;
                r.applied = Some(0);
                Some(0)
            } else if !r.asleep && r.applied != Some(target) {
                r.applied = Some(target);
                Some(target)
            } else {
                None
            }
        };

        if let Some(value) = action {
            set_device_brightness(&id, value).await;
        }
    }
}

/// Tras suspender el PC el deck se reinicia: hay que reenviar el brillo
pub async fn invalidate_all() {
    for r in RUNTIME.lock().await.values_mut() {
        r.applied = None;
        r.asleep = false;
        r.last_activity = Instant::now();
    }
    CHANGED.notify_one();
}
