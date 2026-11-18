//! Lo que pinta una animación en el deck: las teclas y las pantallas de las
//! ruedas como una sola rejilla, y el ritmo al que el deck aguanta los cambios

use std::time::{Duration, Instant};

use image::DynamicImage;
use mirajazz::{device::Device, error::MirajazzError};

use super::color::solid;
use crate::{family::Model, settings::Calibration};

/// Espera lo que necesita el deck tras enviar `keys` teclas (y al menos `min`)
pub(super) async fn pace(model: &Model, since: Instant, keys: usize, min: Duration) {
    let (base, per_key) = model.anim_pacing();
    let need = Duration::from_millis(base + per_key * keys as u64).max(min);
    let spent = since.elapsed();
    if spent < need {
        tokio::time::sleep(need - spent).await;
    }
}

/// Rejilla de las animaciones: las teclas y, en el MSD-PRO, una fila más
/// abajo que son las pantallas de las ruedas
pub(super) fn anim_grid(model: &Model) -> (u8, u8) {
    let (rows, cols) = model.grid();
    if model.strip_zones() > 0 { (rows + 1, cols) } else { (rows, cols) }
}

/// Pantalla de rueda que corresponde a una columna de la fila de abajo
pub(super) fn zone_of(col: u8, cols: u8, zones: u8) -> u8 {
    (((col as f32 + 0.5) / cols as f32) * zones as f32).floor().min(zones as f32 - 1.0) as u8
}

pub(super) fn brightness_of(c: Option<[u8; 3]>) -> u32 {
    c.map_or(0, |c| c.iter().map(|&v| v as u32).sum())
}

/// Lo que pinta una animación: teclas y pantallas de las ruedas. Cada
/// pantalla toma el color más vivo de las columnas que tiene encima en la
/// rejilla y solo se reenvía si cambia.
pub(super) struct Screen<'a> {
    device: &'a Device,
    model: &'a Model,
    cal: &'a Calibration,
    keys: u8,
    cols: u8,
    zones: u8,
    dark: bool,
    strip: Vec<Option<[u8; 3]>>,
    shown: Vec<Option<Option<[u8; 3]>>>,
}

impl<'a> Screen<'a> {
    /// `dark`: lo apagado se pinta en negro en vez de dejar ver el fondo del
    /// deck (al arrancar)
    pub(super) fn new(device: &'a Device, model: &'a Model, cal: &'a Calibration, dark: bool) -> Self {
        let (rows, cols) = model.grid();
        let zones = model.strip_zones();
        Self { device, model, cal, keys: rows * cols, cols, zones, dark, strip: vec![None; cols as usize], shown: vec![None; zones as usize] }
    }

    /// Pinta (Some) o apaga (None) una posición de la rejilla
    pub(super) async fn set(&mut self, pos: u8, c: Option<[u8; 3]>) -> Result<(), MirajazzError> {
        let c = if self.dark { Some(c.unwrap_or([0, 0, 0])) } else { c };
        if pos < self.keys {
            return match c {
                Some(c) => self.model.draw_key(self.device, pos, self.model.calibrate(self.cal, pos, false, solid(c))).await,
                None => self.model.clear_key(self.device, pos, false).await,
            };
        }
        let col = pos - self.keys;
        if self.zones == 0 || col >= self.cols {
            return Ok(());
        }
        self.strip[col as usize] = c;
        let zone = zone_of(col, self.cols, self.zones);
        let best = (0..self.cols)
            .filter(|&x| zone_of(x, self.cols, self.zones) == zone)
            .map(|x| self.strip[x as usize])
            .max_by_key(|&c| brightness_of(c))
            .flatten();
        if self.shown[zone as usize] == Some(best) {
            return Ok(());
        }
        self.shown[zone as usize] = Some(best);
        match best {
            Some(c) => self.model.draw_strip(self.device, zone, self.model.calibrate(self.cal, zone, true, solid(c))).await,
            None => self.model.clear_key(self.device, zone, true).await,
        }
    }
}
/// Todo el deck como un solo lienzo: las teclas y, debajo, una franja para
/// las pantallas de las ruedas (con su proporción)
pub(super) struct Layout {
    pub rows: u32,
    pub cols: u32,
    /// Lado de una tecla
    pub key: u32,
    pub zones: u32,
    /// Ancho del lienzo
    pub w: u32,
    /// Alto de la franja de las ruedas
    pub band: u32,
}

impl Layout {
    pub fn of(model: &Model) -> Self {
        let (rows, cols) = model.grid();
        let key = model.key_size();
        let zones = model.strip_zones() as u32;
        let (sw, sh) = model.strip_size();
        let w = cols as u32 * key;
        let band = w.checked_div(zones).map_or(0, |zone_w| zone_w * sh / sw.max(1));
        Self { rows: rows as u32, cols: cols as u32, key, zones, w, band }
    }

    /// Alto de las teclas
    pub fn keys_h(&self) -> u32 {
        self.rows * self.key
    }

    /// Alto de todo el lienzo
    pub fn h(&self) -> u32 {
        self.keys_h() + self.band
    }
}

/// Reparte un lienzo del tamaño de `Layout` entre las teclas y las pantallas
/// de las ruedas (sin enviarlo todavía: falta `flush`)
pub(super) async fn draw_canvas(device: &Device, model: &Model, cal: &Calibration, l: &Layout, canvas: &DynamicImage) -> Result<(), MirajazzError> {
    for y in 0..l.rows {
        for x in 0..l.cols {
            let pos = (y * l.cols + x) as u8;
            let tile = canvas.crop_imm(x * l.key, y * l.key, l.key, l.key);
            model.draw_key(device, pos, model.calibrate(cal, pos, false, tile)).await?;
        }
    }
    for z in 0..l.zones {
        let tile = canvas.crop_imm(z * (l.w / l.zones), l.keys_h(), l.w / l.zones, l.band);
        model.draw_strip(device, z as u8, model.calibrate(cal, z as u8, true, tile)).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pantallas_de_las_ruedas() {
        assert_eq!((0..5).map(|c| zone_of(c, 5, 4)).collect::<Vec<_>>(), vec![0, 1, 2, 2, 3]);
        let pro = Model::from_vid_pid(0x0b00, 0x1003).unwrap();
        assert_eq!(anim_grid(&pro), (3, 5));
        let one = Model::from_vid_pid(0x0b00, 0x1000).unwrap();
        assert_eq!(anim_grid(&one), one.grid());
        let l = Layout::of(&one);
        assert_eq!((l.band, l.zones), (0, 0));
    }
}
