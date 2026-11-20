//! El deck en negro al arrancar: no se ve el fondo del deck hasta que
//! OpenDeck ha pintado los iconos. Lo que OpenDeck no pinta se borra luego.

use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use mirajazz::{device::Device, error::MirajazzError};

use super::color::solid;
use crate::{family::Model, settings::Calibration};

/// Todo el deck en negro (teclas y pantallas de las ruedas). Al arrancar no
/// se ve el fondo del deck hasta que OpenDeck pinta los iconos.
pub async fn blackout(device: &Device, model: &Model, cal: &Calibration) -> Result<(), MirajazzError> {
    let (rows, cols) = model.grid();
    for pos in 0..rows * cols {
        model.draw_key(device, pos, model.calibrate(cal, pos, false, solid([0, 0, 0]))).await?;
    }
    for z in 0..model.strip_zones() {
        model.draw_strip(device, z, model.calibrate(cal, z, true, solid([0, 0, 0]))).await?;
    }
    device.flush().await
}
/// Teclas pintadas de cada deck: (si es pantalla de rueda, posición)
type Painted = HashMap<String, HashSet<(bool, u8)>>;

/// Teclas que ha pintado OpenDeck desde que arrancó el deck (en negro)
static STARTING: LazyLock<std::sync::Mutex<Painted>> = LazyLock::new(Default::default);

/// El deck arranca en negro: se apunta lo que pinta OpenDeck
pub fn start_dark(id: &str) {
    STARTING.lock().unwrap().insert(id.to_string(), HashSet::new());
}

/// OpenDeck ha pintado (o borrado) esta tecla
pub fn painted(id: &str, encoder: bool, pos: u8) {
    if let Some(s) = STARTING.lock().unwrap().get_mut(id) {
        s.insert((encoder, pos));
    }
}

/// Pasado el arranque, las teclas que OpenDeck no ha pintado (vacías) se
/// borran para que se vea el fondo del deck, como siempre
pub async fn end_dark(id: &str) {
    let Some(done) = STARTING.lock().unwrap().remove(id) else { return };
    let devices = crate::DEVICES.read().await;
    let Some(device) = devices.get(id) else { return };
    let Some(model) = Model::from_vid_pid(device.vid, device.pid) else { return };
    let (rows, cols) = model.grid();
    for pos in 0..rows * cols {
        if !done.contains(&(false, pos)) {
            model.clear_key(device, pos, false).await.ok();
        }
    }
    for z in 0..model.strip_zones() {
        if !done.contains(&(true, z)) {
            model.clear_key(device, z, true).await.ok();
        }
    }
    device.flush().await.ok();
}
