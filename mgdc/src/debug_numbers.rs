//! PRUEBAS — BORRAR CUANDO NO HAGA FALTA.
//!
//! Numera las pantallas de los botones en el mismo orden en que la cascada
//! ("Tus iconos") las saca de su HashMap y las pinta, para ver cómo está
//! organizado cada deck. En cada tecla se dibuja:
//!   - grande: el orden de pintado (0, 1, 2…)
//!   - abajo:  "OD<n>" = posición en OpenDeck, "HW<n>" = índice real en el deck
//!
//! Para quitarlo: borra este fichero, la línea `mod debug_numbers;` de main.rs,
//! el `.merge(crate::debug_numbers::routes())` de panel.rs y el bloque
//! "PRUEBAS" de panel/index.html.

use std::time::Duration;

use axum::{Json, Router, http::HeaderMap, response::IntoResponse, routing::post};
use image::{DynamicImage, Rgb, RgbImage};
use mirajazz::{device::Device, error::MirajazzError};
use serde::Deserialize;
use serde_json::json;

use crate::{
    DEVICES, animation,
    family::{Model, akp05, akp153},
    panel,
};

/// Pausa entre número y número
const STEP: Duration = Duration::from_millis(500);

/// Una pantalla del deck: posición en OpenDeck y si es de la pantalla de las ruedas
#[derive(Clone, Copy, Debug)]
struct Slot {
    position: u8,
    encoder: bool,
}

/// Mismo orden que usa la cascada (`reveal::order`): primero las teclas por
/// posición de OpenDeck (filas de izquierda a derecha), luego las ruedas.
fn slots(model: &Model) -> Vec<Slot> {
    let l = model.layout();
    let mut v: Vec<Slot> = (0..l.rows * l.cols).map(|p| Slot { position: p, encoder: false }).collect();
    v.extend((0..l.encoders as u8).map(|p| Slot { position: p, encoder: true }));
    v
}

/// Índice que usa el propio deck (el que va en el protocolo USB)
fn hardware_index(model: &Model, s: Slot) -> Result<u8, MirajazzError> {
    match model {
        Model::Akp153(_) => Ok(akp153::opendeck_to_device(s.position)),
        Model::Akp05(_) => akp05::map_position(s.position, s.encoder),
    }
}

async fn draw(device: &Device, model: &Model, s: Slot, order: usize) -> Result<(), MirajazzError> {
    let hw = hardware_index(model, s)?;
    let label = format!("OD{} HW{}", s.position, hw);
    match (model, s.encoder) {
        (Model::Akp05(k), true) => {
            let f = k.touch_image_format();
            let img = number_image(f.size.0 as u32, f.size.1 as u32, order, &label, [40, 20, 70]);
            device.set_button_image(hw, f, img).await
        }
        _ => {
            let size = model.key_size();
            let img = number_image(size, size, order, &label, [10, 30, 70]);
            model.draw_key(device, s.position, img).await
        }
    }
}

// ------------------------------------------------------------- endpoints --

#[derive(Deserialize)]
struct Body {
    id: String,
}

/// Pone los números de uno en uno, cada 0,5 s
async fn number(headers: HeaderMap, Json(b): Json<Body>) -> impl IntoResponse {
    if !panel::origin_ok(&headers) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    animation::set_busy(&b.id, true).await;
    tokio::time::sleep(Duration::from_millis(150)).await;

    let model = match DEVICES.read().await.get(&b.id) {
        Some(d) => Model::from_vid_pid(d.vid, d.pid),
        None => None,
    };
    let Some(model) = model else {
        return Json(json!({ "error": "El deck no está conectado" })).into_response();
    };

    let list = slots(&model);
    let mut table = vec![];
    for (i, s) in list.iter().enumerate() {
        let r = {
            let devices = DEVICES.read().await;
            let Some(device) = devices.get(&b.id) else { break };
            match draw(device, &model, *s, i).await {
                Ok(()) => device.flush().await,
                Err(e) => Err(e),
            }
        };
        if let Err(e) = r {
            return Json(json!({ "error": e.to_string() })).into_response();
        }
        let hw = hardware_index(&model, *s).unwrap_or(255);
        log::info!("Numerar {}: {} -> OpenDeck {} {} -> deck {}", b.id, i, s.position, if s.encoder { "(rueda)" } else { "" }, hw);
        table.push(json!({ "order": i, "opendeck": s.position, "encoder": s.encoder, "hardware": hw }));
        tokio::time::sleep(STEP).await;
    }
    Json(json!({ "ok": true, "table": table })).into_response()
}

/// Quita los números (todas las pantallas en negro)
async fn clear(headers: HeaderMap, Json(b): Json<Body>) -> impl IntoResponse {
    if !panel::origin_ok(&headers) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    animation::set_busy(&b.id, true).await;
    if let Some(device) = DEVICES.read().await.get(&b.id) {
        animation::clear(device).await.ok();
    }
    Json(json!({ "ok": true })).into_response()
}

/// Vuelve a dejar los iconos de OpenDeck
async fn restore(headers: HeaderMap, Json(b): Json<Body>) -> impl IntoResponse {
    if !panel::origin_ok(&headers) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    animation::set_busy(&b.id, false).await;
    panel::rerender(&b.id).await;
    Json(json!({ "ok": true })).into_response()
}

pub fn routes() -> Router {
    Router::new()
        .route("/api/debug/number", post(number))
        .route("/api/debug/clear", post(clear))
        .route("/api/debug/restore", post(restore))
}

// ------------------------------------------------------- dibujar números --

/// Letra de 5×7 píxeles (cada fila: 5 bits, el de la izquierda es el más alto)
fn glyph(c: char) -> [u8; 7] {
    match c {
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A],
        _ => [0; 7],
    }
}

fn text_width(text: &str, scale: u32) -> u32 {
    (text.chars().count() as u32 * 6).saturating_sub(1) * scale
}

fn draw_text(img: &mut RgbImage, text: &str, x0: i64, y0: i64, scale: u32, color: [u8; 3]) {
    for (i, c) in text.chars().enumerate() {
        let g = glyph(c);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (0x10 >> col) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = x0 + ((i as u32 * 6 + col) * scale + dx) as i64;
                        let y = y0 + (row as u32 * scale + dy) as i64;
                        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                            img.put_pixel(x as u32, y as u32, Rgb(color));
                        }
                    }
                }
            }
        }
    }
}

/// Fondo de color con borde, el número grande en el centro y la etiqueta abajo
pub fn number_image(w: u32, h: u32, n: usize, label: &str, bg: [u8; 3]) -> DynamicImage {
    let mut img = RgbImage::from_pixel(w, h, Rgb(bg));
    let border = (w.min(h) / 28).max(1);
    for (x, y, p) in img.enumerate_pixels_mut() {
        if x < border || y < border || x >= w - border || y >= h - border {
            *p = Rgb([255, 200, 0]);
        }
    }
    let big = n.to_string();
    let scale = (h * 45 / 100 / 7).max(1);
    let bx = (w as i64 - text_width(&big, scale) as i64) / 2;
    let by = (h as i64 * 12 / 100).max(border as i64 + 1);
    draw_text(&mut img, &big, bx, by, scale, [255, 255, 255]);

    let small = (h / 85).max(1) + if h >= 100 { 1 } else { 0 };
    let lx = (w as i64 - text_width(label, small) as i64) / 2;
    let ly = h as i64 - (7 * small) as i64 - (h as i64 * 10 / 100);
    draw_text(&mut img, label, lx, ly, small, [255, 200, 0]);
    DynamicImage::ImageRgb8(img)
}

#[cfg(test)]
#[test]
#[ignore]
fn guardar_numeros() {
    let dir = std::env::var("MGDC_NUM_OUT").unwrap();
    number_image(85, 85, 12, "OD0 HW12", [10, 30, 70]).resize(255, 255, image::imageops::FilterType::Nearest).save(format!("{dir}/n85.png")).unwrap();
    number_image(176, 112, 14, "OD0 HW10", [40, 20, 70]).resize(528, 336, image::imageops::FilterType::Nearest).save(format!("{dir}/n176.png")).unwrap();
}
