//! Dibujo de la rueda de perfiles. OpenDeck no pinta imágenes dentro del SVG,
//! así que el icono del perfil se pega encima de lo que manda OpenDeck antes de
//! mandarlo al deck (y, si es un GIF, se mueve desde aquí).

use std::{collections::HashMap, sync::LazyLock, time::{Duration, Instant}};

use openaction::SetImageEvent;
use tokio::sync::Mutex;

use crate::svg::esc;

use super::{
    nav::view,
    wheel::{Shown, WHEELS},
};

/// Última pantalla de cada rueda de perfiles tal como la dibuja OpenDeck (sin
/// icono): OpenDeck no pinta imágenes dentro del SVG, así que el icono se pega
/// aquí encima antes de mandarla al deck
pub(super) static BASES: LazyLock<Mutex<HashMap<(String, u8), image::DynamicImage>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

// ------------------------------------------------------------------ dibujo --

pub(super) async fn redraw(context: &str) {
    let svg = render(context).await;
    crate::canvas::send(context, true, &svg).await;
}

pub async fn redraw_device(device: &str) {
    let contexts: Vec<String> =
        WHEELS.lock().await.iter().filter(|(_, w)| w.device == device).map(|(c, _)| c.clone()).collect();
    for c in contexts {
        redraw(&c).await;
    }
    crate::profile_keys::refresh_device(device).await;
}

pub(super) async fn render(context: &str) -> String {
    let Some((device, direct)) = WHEELS.lock().await.get(context).map(|w| (w.device.clone(), w.direct)) else {
        return render_empty();
    };
    let v = view(&device).await;
    let Some(target) = v.selected.clone() else { return render_empty() };
    let active = v.active.as_deref() == Some(target.as_str());
    let shown = Shown { target: Some(target.clone()), at: v.at + 1, total: v.list.len() };
    if let Some(w) = WHEELS.lock().await.get_mut(context) {
        w.shown = Some(shown.clone());
    }
    let border = {
        let d = device.clone();
        tokio::task::spawn_blocking(move || crate::profile_icons::border(&d)).await.unwrap_or_else(|_| crate::profile_icons::DEFAULT_BORDER.into())
    };
    let kids = v.kids(&target).len();
    let badge = match kids {
        0 => String::new(),
        1 => crate::i18n::t("profiles.subprofile_one").into(),
        k => crate::i18n::tf("profiles.subprofiles", &[&k]),
    };
    render_slot(&SlotView {
        title: &v.level_label(),
        name: Some(&v.name(&target)),
        badge: &badge,
        at: shown.at,
        total: shown.total,
        active,
        direct,
        border: &border,
    })
}

fn started() -> &'static Instant {
    static START: LazyLock<Instant> = LazyLock::new(Instant::now);
    &START
}

/// Lo que va en el recuadro: el icono del perfil (del panel de Prisma) en el
/// segundo `t`. Sin icono no se pega nada: se ve solo el nombre.
async fn picture(device: &str, s: &Shown, t: f32) -> Option<image::RgbaImage> {
    let (d, target) = (device.to_string(), s.target.clone());
    tokio::task::spawn_blocking(move || target.and_then(|p| crate::profile_icons::frame(&d, &p, t)).map(|f| (*f).clone()))
        .await
        .ok()
        .flatten()
}

fn jpeg_url(img: &image::RgbaImage) -> Option<String> {
    let mut out = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(img.clone()).to_rgb8().write_to(&mut out, image::ImageFormat::Jpeg).ok()?;
    Some(format!("data:image/jpeg;base64,{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, out.into_inner())))
}

/// OpenDeck manda la pantalla de una rueda: si es una rueda de perfiles, se
/// guarda tal cual y se le pega el icono del perfil elegido
pub async fn compose(mut evt: SetImageEvent) -> SetImageEvent {
    if evt.controller.as_deref() != Some("Encoder") {
        return evt;
    }
    let (Some(pos), Some(url)) = (evt.position, evt.image.clone()) else { return evt };
    let wheel = WHEELS.lock().await.values().find(|w| w.device == evt.device && w.position == pos).map(|w| w.shown.clone());
    let Some(shown) = wheel else { return evt };
    let Some(base) = data_url::DataUrl::process(&url)
        .ok()
        .and_then(|u| u.decode_to_vec().ok())
        .and_then(|(b, _)| image::load_from_memory(&b).ok())
    else {
        return evt;
    };
    BASES.lock().await.insert((evt.device.clone(), pos), base.clone());
    let Some(shown) = shown else { return evt };
    if let Some(pic) = picture(&evt.device, &shown, started().elapsed().as_secs_f32()).await {
        let d = crate::profile_icons::paste(&base, &pic);
        if let Some(u) = jpeg_url(&d) {
            evt.image = Some(u);
        }
    }
    evt
}

/// Mueve los iconos GIF de las ruedas de perfiles (los pinta Prisma directamente,
/// sin pasar por OpenDeck)
pub async fn anim_task() {
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let wheels: Vec<(String, u8, Shown)> = WHEELS
            .lock()
            .await
            .values()
            .filter_map(|w| Some((w.device.clone(), w.position, w.shown.clone()?)))
            .collect();
        for (device, position, shown) in wheels {
            let Some(target) = shown.target.clone() else { continue };
            let d = device.clone();
            let moving = tokio::task::spawn_blocking(move || crate::profile_icons::animated(&d, &target)).await.unwrap_or(false);
            if !moving {
                continue;
            }
            let Some(base) = BASES.lock().await.get(&(device.clone(), position)).cloned() else { continue };
            let Some(pic) = picture(&device, &shown, started().elapsed().as_secs_f32()).await else { continue };
            let Some(url) = jpeg_url(&crate::profile_icons::paste(&base, &pic)) else { continue };
            crate::paint(SetImageEvent { device, controller: Some("Encoder".into()), position: Some(position), image: Some(url) }).await;
        }
    }
}

/// Pantalla de la rueda de perfiles: el recuadro (el icono se pega luego
/// encima, ver `compose`), el número en la lista y qué hace pulsar. El activo
/// lleva el marco y el texto en el color `border`. Sin icono ni imagen se ve
/// el nombre del perfil dentro del recuadro.
#[derive(Clone, Copy)]
pub struct SlotView<'a> {
    /// Nivel: "PERFILES", "AVIONES ›"…
    pub title: &'a str,
    /// Nombre del perfil (se ve si no tiene icono)
    pub name: Option<&'a str>,
    /// Subperfiles ("▸ 2 subperfiles") o nada
    pub badge: &'a str,
    /// Posición en la lista (desde 1) y cuántos hay
    pub at: usize,
    pub total: usize,
    /// Es el perfil activo
    pub active: bool,
    /// La rueda cambia al soltar (si no, al pulsar)
    pub direct: bool,
    /// Color del marco del activo ("#2ecc71")
    pub border: &'a str,
}

pub fn render_slot(v: &SlotView) -> String {
    let SlotView { title, name, badge, at, total, active, direct, border } = *v;
    let state = if active {
        crate::i18n::t("profiles.wheel_active")
    } else if direct {
        crate::i18n::t("profiles.wheel_on_release")
    } else {
        crate::i18n::t("profiles.wheel_press")
    };
    let border = if border.len() == 7 && border.starts_with('#') && border[1..].chars().all(|c| c.is_ascii_hexdigit()) {
        border
    } else {
        crate::profile_icons::DEFAULT_BORDER
    };
    // Sin icono se ve el nombre: cuanto más corto, más grande
    let name_len = name.unwrap_or("").chars().count();
    let name_size = if name_len <= 5 { 18 } else if name_len <= 7 { 14 } else if name_len <= 9 { 12 } else { 10 };
    let name = esc(&name.unwrap_or("").chars().take(12).collect::<String>());
    let title = esc(&title.to_uppercase().chars().take(16).collect::<String>());
    let badge = esc(badge);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100"><rect width="200" height="100" fill="#000"/><rect x="10" y="14" width="72" height="72" rx="8" fill="#222"/><text x="46" y="{name_y}" font-family="sans-serif" font-size="{name_size}" font-weight="bold" fill="#ddd" text-anchor="middle">{name}</text><rect x="10" y="14" width="72" height="72" rx="8" fill="none" stroke="{frame}" stroke-width="{width}"/><text x="96" y="28" font-family="sans-serif" font-size="9" letter-spacing="1.2" fill="#999">{title}</text><text x="96" y="58" font-family="sans-serif" font-size="28" font-weight="bold" fill="#fff">{at}<tspan font-size="17" fill="#888"> / {total}</tspan></text><text x="96" y="73" font-family="sans-serif" font-size="9" fill="#f0b030">{badge}</text><text x="96" y="88" font-family="sans-serif" font-size="8" letter-spacing="0.6" fill="{state_color}">{state}</text></svg>"##,
        name_y = 50 + name_size * 7 / 20,
        frame = if active { border } else { "#444" },
        width = if active { 3 } else { 2 },
        state_color = if active { border } else { "#bbb" },
    )
}

fn render_empty() -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100"><rect width="200" height="100" fill="#000"/><text x="100" y="36" font-family="sans-serif" font-size="13" font-weight="bold" fill="#fff" text-anchor="middle">{}</text><text font-family="sans-serif" font-size="10" fill="#bbb" text-anchor="middle"><tspan x="100" y="58">{}</tspan><tspan x="100" y="72">{}</tspan></text></svg>"##,
        crate::i18n::t("profiles.none_title"),
        crate::i18n::t("profiles.none_hint1"),
        crate::i18n::t("profiles.none_hint2")
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dibujos() {
        assert!(render_slot(&SlotView { title: "Perfiles", name: None, badge: "", at: 2, total: 5, active: false, direct: false, border: "#2ecc71" }).contains("PULSA"));
        assert!(render_slot(&SlotView { title: "aviones ›", name: Some("F-18"), badge: "▸ 2 subperfiles", at: 1, total: 3, active: true, direct: true, border: "#ff9f1a" }).contains("#ff9f1a"));
        assert!(render_empty().contains("Sin perfiles"));
    }

/// PRISMA_SAMPLES_OUT=carpeta cargo test muestras_perfiles -- --ignored
#[cfg(test)]
#[test]
#[ignore]
fn muestras_perfiles() {
    let dir = std::env::var("PRISMA_SAMPLES_OUT").unwrap();
    let s = [
        ("p1-elegir", render_slot(&SlotView { title: "Perfiles", name: Some("Juegos"), badge: "", at: 2, total: 4, active: false, direct: false, border: "#2ecc71" })),
        ("p2-activo", render_slot(&SlotView { title: "Perfiles", name: Some("Música"), badge: "", at: 1, total: 4, active: true, direct: false, border: "#2ecc71" })),
        ("p3-directo", render_slot(&SlotView { title: "Perfiles", name: Some("Trabajo"), badge: "", at: 3, total: 4, active: false, direct: true, border: "#2ecc71" })),
        ("p4-vacio", render_empty()),
    ];
    for (n, svg) in s {
        std::fs::write(format!("{dir}/{n}.svg"), svg).unwrap();
    }
}

/// PRISMA_SAMPLES_OUT=carpeta cargo test muestras_niveles -- --ignored
#[cfg(test)]
#[test]
#[ignore]
fn muestras_niveles() {
    let dir = std::env::var("PRISMA_SAMPLES_OUT").unwrap();
    let s = [
        ("w-raiz", render_slot(&SlotView { title: "Perfiles", name: Some("aviones"), badge: "▸ 2 subperfiles", at: 2, total: 3, active: false, direct: false, border: "#2ecc71" })),
        ("w-sub", render_slot(&SlotView { title: "aviones ›", name: Some("Cessna"), badge: "", at: 2, total: 2, active: true, direct: false, border: "#2ecc71" })),
    ];
    for (n, svg) in s {
        std::fs::write(format!("{dir}/{n}.svg"), svg).unwrap();
    }
}
}
