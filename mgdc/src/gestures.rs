//! Gestos de la pantalla táctil del MSD-PRO.
//!
//! El deck manda los toques de la pantalla con el mismo evento que pulsar la
//! rueda de debajo, así que al leerlos se apunta aquí que ha sido un toque;
//! la rueda de perfiles lo mira para saber si fue pantalla o pulsador.
//! Los deslizamientos a izquierda/derecha llegan como el giro de una rueda de
//! más (STRIP), que no se manda a OpenDeck: la rueda de perfiles lo usa como
//! si se girara el knob.

use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

/// Rueda "de mentira" para los deslizamientos de la pantalla
pub const STRIP: u8 = 4;

static LAST_TOUCH: Mutex<Option<(usize, Instant)>> = Mutex::new(None);

/// Apunta que la pantalla se ha tocado encima de la rueda `encoder`
pub fn mark_touch(encoder: usize) {
    *LAST_TOUCH.lock().unwrap() = Some((encoder, Instant::now()));
}

/// Si la última pulsación de la rueda `encoder` fue un toque de la pantalla
/// (y se olvida)
pub fn take_touch(encoder: usize) -> bool {
    let mut last = LAST_TOUCH.lock().unwrap();
    let hit = last.is_some_and(|(e, t)| e == encoder && t.elapsed() < Duration::from_millis(500));
    if hit {
        *last = None;
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toque_o_pulsador() {
        mark_touch(3);
        assert!(!take_touch(1));
        assert!(take_touch(3));
        assert!(!take_touch(3));
    }
}
