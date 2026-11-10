//! Animación "cascada": tus propios iconos aparecen tecla a tecla al arrancar
//! y se van tecla a tecla al cerrar.
//!
//! Al arrancar, las imágenes que manda OpenDeck se guardan un momento y luego
//! se pintan de una en una, al ritmo que aguanta cada deck (el MSD-ONE
//! incluido, porque es lo mismo que hace OpenDeck al repintar).

use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

use mirajazz::error::MirajazzError;
use openaction::SetImageEvent;
use tokio::sync::Mutex;

use crate::{DEVICES, family::{self, Model}};

struct Pending {
    events: Vec<SetImageEvent>,
    last: Instant,
}

static PENDING: LazyLock<Mutex<HashMap<String, Pending>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Si hay una cascada en marcha para ese deck, se queda la imagen y devuelve `None`
pub async fn intercept(evt: SetImageEvent) -> Option<SetImageEvent> {
    let mut p = PENDING.lock().await;
    match p.get_mut(&evt.device) {
        Some(state) => {
            state.events.push(evt);
            state.last = Instant::now();
            None
        }
        None => Some(evt),
    }
}

/// Pausa entre tecla y tecla: el total repartido, pero nunca más rápido de lo que aguanta el deck
fn step(model: &Model, duration_ms: u32, n: usize) -> Duration {
    let (base, per_key) = model.anim_pacing();
    let min = base + per_key;
    let even = duration_ms.clamp(300, 8000) as u64 / n.max(1) as u64;
    Duration::from_millis(even.max(min))
}

/// Orden de pintado: primero las teclas por filas, luego la pantalla de las ruedas
fn order(evt: &SetImageEvent) -> (u8, u8) {
    let enc = evt.controller.as_deref() == Some("Encoder");
    (enc as u8, evt.position.unwrap_or(0))
}

/// Empieza a guardar las imágenes de OpenDeck y, cuando dejan de llegar,
/// las pinta de una en una. Termina cuando la cascada ha acabado.
pub async fn appear(id: String, model: Model, duration_ms: u32) {
    let start = Instant::now();
    PENDING.lock().await.insert(id.clone(), Pending { events: vec![], last: Instant::now() });

    // Esperar a que OpenDeck mande todas las imágenes (o 4 s como mucho)
    loop {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let p = PENDING.lock().await;
        let Some(s) = p.get(&id) else { return };
        let quiet = s.last.elapsed() > Duration::from_millis(300);
        if (!s.events.is_empty() && quiet) || start.elapsed() > Duration::from_secs(4) {
            break;
        }
    }

    // Quedarse con la última imagen de cada tecla, en orden
    let mut latest: HashMap<(u8, u8), SetImageEvent> = HashMap::new();
    let mut clear_all = None;
    for e in PENDING.lock().await.get_mut(&id).map(|s| std::mem::take(&mut s.events)).unwrap_or_default() {
        if e.position.is_none() {
            clear_all = Some(e);
            latest.clear();
        } else {
            latest.insert(order(&e), e);
        }
    }
    let mut events: Vec<_> = latest.into_values().collect();
    events.sort_by_key(order);
    let pause = step(&model, duration_ms, events.len());
    log::info!("Cascada de {id}: {} iconos, {} ms entre tecla y tecla", events.len(), pause.as_millis());

    if let Some(e) = clear_all {
        paint(&id, e).await.ok();
    }
    for e in events {
        let t = Instant::now();
        if paint(&id, e).await.is_err() {
            break;
        }
        if let Some(rest) = pause.checked_sub(t.elapsed()) {
            tokio::time::sleep(rest).await;
        }
    }

    // Lo que haya llegado mientras tanto (p. ej. LHMonitor), al final y de golpe
    let late = PENDING.lock().await.remove(&id).map(|s| s.events).unwrap_or_default();
    for e in late {
        paint(&id, e).await.ok();
    }
}

async fn paint(id: &str, evt: SetImageEvent) -> Result<(), MirajazzError> {
    let devices = DEVICES.read().await;
    let Some(device) = devices.get(id) else { return Ok(()) };
    family::set_image(device, id, evt).await?;
    device.flush().await
}

/// Cierre: las teclas se van apagando de una en una (en orden inverso)
pub async fn disappear(device: &mirajazz::device::Device, model: &Model, duration_ms: u32) -> Result<(), MirajazzError> {
    let layout = model.layout();
    let mut keys: Vec<(u8, bool)> = (0..layout.encoders as u8).rev().map(|e| (e, true)).collect();
    keys.extend((0..(layout.rows * layout.cols)).rev().map(|k| (k, false)));
    let pause = step(model, duration_ms, keys.len());
    for (pos, enc) in keys {
        let t = Instant::now();
        model.clear_key(device, pos, enc).await?;
        device.flush().await?;
        if let Some(rest) = pause.checked_sub(t.elapsed()) {
            tokio::time::sleep(rest).await;
        }
    }
    Ok(())
}
