//! Teclas de perfiles:
//! - «Entrar / cambiar» (GO): cambia al perfil elegido con la rueda (o al que
//!   tenga fijado en sus ajustes) y, si tiene subperfiles, la rueda pasa a
//!   ellos. Mantenerla pulsada (0,6 s) sube un nivel.
//! - «Perfil actual» (NOW): solo enseña el perfil elegido (y si es el activo).
//!   Pulsarla no hace nada.
//!
//! El estado (nivel y elegido) es el del deck, el mismo que usa la rueda (ver
//! profiles.rs).

use base64::Engine;
use openaction::OutboundEventManager;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use crate::svg::{clip, esc};

pub const GO: &str = "com.malzar.prisma.profilego";
pub const NOW: &str = "com.malzar.prisma.profilenow";
/// Lo que hay que mantener pulsada «Entrar / cambiar» para subir un nivel
const HOLD: Duration = Duration::from_millis(600);

pub fn is_mine(action: &str) -> bool {
    action == GO || action == NOW
}

#[derive(Clone, Copy, PartialEq)]
enum Show {
    IconName,
    Icon,
    Name,
}

struct Key {
    device: String,
    go: bool,
    /// Perfil fijo (None = el elegido con la rueda)
    profile: Option<String>,
    show: Show,
    down: Option<Instant>,
    last: String,
}

static KEYS: LazyLock<Mutex<HashMap<String, Key>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn read_settings(s: &serde_json::Value) -> (Option<String>, Show) {
    let profile = s.get("profile").and_then(|p| p.as_str()).filter(|p| !p.is_empty()).map(String::from);
    let show = match s.get("show").and_then(|p| p.as_str()) {
        Some("icon") => Show::Icon,
        Some("name") => Show::Name,
        _ => Show::IconName,
    };
    (profile, show)
}

pub async fn appear(context: &str, device: &str, action: &str, settings: &serde_json::Value, outbound: &mut OutboundEventManager) {
    let (profile, show) = read_settings(settings);
    KEYS.lock().await.insert(
        context.to_string(),
        Key { device: device.to_string(), go: action == GO, profile, show, down: None, last: String::new() },
    );
    if let Some(svg) = render(context).await {
        crate::canvas::send_with(outbound, context, false, &svg).await;
    }
}

pub async fn disappear(context: &str) {
    KEYS.lock().await.remove(context);
}

pub async fn settings_changed(context: &str, settings: &serde_json::Value) {
    let (profile, show) = read_settings(settings);
    if let Some(k) = KEYS.lock().await.get_mut(context) {
        k.profile = profile;
        k.show = show;
        k.last.clear();
    }
    refresh(context).await;
}

pub async fn key_down(context: &str) {
    if let Some(k) = KEYS.lock().await.get_mut(context) {
        k.down = Some(Instant::now());
    }
}

pub async fn key_up(context: &str) {
    let Some((device, go, fixed, held)) = KEYS.lock().await.get_mut(context).map(|k| {
        let held = k.down.take().is_some_and(|t| t.elapsed() >= HOLD);
        (k.device.clone(), k.go, k.profile.clone(), held)
    }) else {
        return;
    };
    // «Perfil actual» solo enseña
    if !go {
        return;
    }
    if held {
        crate::profiles::up(&device).await;
        return;
    }
    let target = match fixed {
        Some(p) => Some(p),
        None => crate::profiles::view(&device).await.selected,
    };
    if let Some(t) = target {
        crate::profiles::go(&device, &t, true).await;
    }
}

/// La ventana de ajustes de «Entrar / cambiar» pide los perfiles (con su nivel)
pub async fn send_tree(context: &str) {
    let Some(device) = KEYS.lock().await.get(context).map(|k| k.device.clone()) else { return };
    let v = crate::profiles::view(&device).await;
    let list: Vec<serde_json::Value> = crate::hierarchy::flatten(&v.tree)
        .into_iter()
        .map(|(id, depth)| json!({ "id": id, "name": v.name(&id), "depth": depth }))
        .collect();
    crate::canvas::reply(context, json!({ "tree": list })).await;
}

/// Repinta las teclas de perfiles de un deck (si ha cambiado algo)
pub async fn refresh_device(device: &str) {
    let contexts: Vec<String> = KEYS.lock().await.iter().filter(|(_, k)| k.device == device).map(|(c, _)| c.clone()).collect();
    for c in contexts {
        refresh(&c).await;
    }
}

async fn refresh(context: &str) {
    let Some(svg) = render(context).await else { return };
    {
        let mut keys = KEYS.lock().await;
        let Some(k) = keys.get_mut(context) else { return };
        if k.last == svg {
            return;
        }
        k.last = svg.clone();
    }
    crate::canvas::send(context, false, &svg).await;
}

/// Repinta cuando el perfil activo cambia por fuera (OpenDeck, enlaces…)
pub async fn watch_task() {
    let mut last: HashMap<String, Option<String>> = HashMap::new();
    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let devices: Vec<String> = {
            let keys = KEYS.lock().await;
            let mut d: Vec<String> = keys.values().map(|k| k.device.clone()).collect();
            d.sort();
            d.dedup();
            d
        };
        for device in devices {
            let d = device.clone();
            let active = tokio::task::spawn_blocking(move || crate::profiles::active_profile(&d)).await.ok().flatten();
            if last.insert(device.clone(), active.clone()) != Some(active) {
                crate::profiles::redraw_device(&device).await;
            }
        }
    }
}

async fn render(context: &str) -> Option<String> {
    let (device, go, fixed, show) = KEYS.lock().await.get(context).map(|k| (k.device.clone(), k.go, k.profile.clone(), k.show))?;
    let v = crate::profiles::view(&device).await;
    let d = device.clone();
    let border = tokio::task::spawn_blocking(move || crate::profile_icons::border(&d)).await.unwrap_or_else(|_| crate::profile_icons::DEFAULT_BORDER.into());
    if !go {
        let sel = v.selected.clone();
        let (top, name, tag, tag_color) = match &sel {
            None => ("".to_string(), "—".to_string(), crate::i18n::t("profiles.none").to_string(), "#888"),
            Some(s) => {
                let top = match crate::hierarchy::parent_of(&v.tree, s) {
                    Some(p) => format!("{} ›", v.name(&p)),
                    None => crate::i18n::t("profiles.root").into(),
                };
                let active = v.active.as_deref() == Some(s.as_str());
                (top, v.name(s), if active { crate::i18n::t("profiles.active").into() } else { crate::i18n::t("profiles.selected").into() }, if active { border.as_str() } else { "#bbb" })
            }
        };
        return Some(render_now(&top, &name, &tag, tag_color));
    }
    let target = fixed.clone().or(v.selected.clone());
    let Some(t) = target else { return Some(render_go(None, "—", "", false, &border)) };
    let active = v.active.as_deref() == Some(t.as_str());
    let kids = v.kids(&t).len();
    let hint = if kids > 0 { crate::i18n::tf("profiles.open", &[&kids]) } else { String::new() };
    let icon = if show == Show::Name {
        None
    } else {
        let (d, p) = (device.clone(), t.clone());
        tokio::task::spawn_blocking(move || crate::profile_icons::frame(&d, &p, 0.0).and_then(|f| png_url(&f))).await.ok().flatten()
    };
    let name = if show == Show::Icon && icon.is_some() { String::new() } else { v.name(&t) };
    Some(render_go(icon.as_deref(), &name, &hint, active, &border))
}

fn png_url(img: &image::RgbaImage) -> Option<String> {
    let mut out = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(img.clone()).write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.into_inner())))
}

/// «Entrar / cambiar»: icono (si hay) y nombre del perfil al que va
pub fn render_go(icon: Option<&str>, name: &str, hint: &str, active: bool, border: &str) -> String {
    let frame = if active {
        format!(r##"<rect x="3" y="3" width="138" height="138" rx="14" fill="none" stroke="{}" stroke-width="6"/>"##, esc(border))
    } else {
        r##"<rect x="3" y="3" width="138" height="138" rx="14" fill="none" stroke="#444" stroke-width="3"/>"##.to_string()
    };
    let name = esc(&clip(name, 12));
    let hint = esc(hint);
    let body = match (icon, name.is_empty()) {
        (Some(url), true) => format!(r##"<image x="20" y="20" width="104" height="104" href="{}" preserveAspectRatio="xMidYMid slice"/>"##, esc(url)),
        (Some(url), false) => format!(
            r##"<image x="32" y="14" width="80" height="80" href="{}" preserveAspectRatio="xMidYMid slice"/><text x="72" y="116" font-family="sans-serif" font-size="18" font-weight="bold" fill="#fff" text-anchor="middle">{name}</text><text x="72" y="133" font-family="sans-serif" font-size="11" fill="#f0b030" text-anchor="middle">{hint}</text>"##,
            esc(url)
        ),
        (None, _) => format!(
            r##"<text x="72" y="60" font-family="sans-serif" font-size="38" fill="#fff" text-anchor="middle">⏎</text><text x="72" y="98" font-family="sans-serif" font-size="20" font-weight="bold" fill="#fff" text-anchor="middle">{name}</text><text x="72" y="122" font-family="sans-serif" font-size="12" fill="#f0b030" text-anchor="middle">{hint}</text>"##
        ),
    };
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 144 144" width="144" height="144"><rect width="144" height="144" fill="#000"/>{body}{frame}</svg>"##)
}

/// «Perfil actual»: texto en un recuadro
pub fn render_now(top: &str, name: &str, tag: &str, tag_color: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 144 144" width="144" height="144"><rect width="144" height="144" fill="#000"/><rect x="10" y="10" width="124" height="124" rx="10" fill="#1b1b1b" stroke="#555" stroke-width="2"/><text x="72" y="44" font-family="sans-serif" font-size="14" fill="#999" text-anchor="middle">{top}</text><text x="72" y="80" font-family="sans-serif" font-size="24" font-weight="bold" fill="#fff" text-anchor="middle">{name}</text><text x="72" y="110" font-family="sans-serif" font-size="14" fill="{color}" text-anchor="middle">{tag}</text></svg>"##,
        top = esc(&clip(top, 14)),
        name = esc(&clip(name, 10)),
        tag = esc(tag),
        color = esc(tag_color),
    )
}

/// El grupo de las teclas «Entrar / cambiar» y «Perfil actual»
pub struct Keys;

impl crate::actions::ActionGroup for Keys {
    fn owns(&self, action: &str) -> bool {
        is_mine(action)
    }
    fn appear<'a>(&'a self, e: &'a openaction::AppearEvent, outbound: &'a mut OutboundEventManager) -> crate::actions::Fut<'a> {
        Box::pin(appear(&e.context, &e.device, &e.action, &e.payload.settings, outbound))
    }
    fn disappear<'a>(&'a self, e: &'a openaction::AppearEvent) -> crate::actions::Fut<'a> {
        Box::pin(disappear(&e.context))
    }
    fn settings<'a>(&'a self, e: &'a openaction::DidReceiveSettingsEvent) -> crate::actions::Fut<'a> {
        let (context, settings) = (e.context.clone(), e.payload.settings.clone());
        tokio::spawn(async move { settings_changed(&context, &settings).await });
        crate::actions::done()
    }
    fn key_down<'a>(&'a self, e: &'a openaction::KeyEvent) -> crate::actions::Fut<'a> {
        let context = e.context.clone();
        tokio::spawn(async move { key_down(&context).await });
        crate::actions::done()
    }
    fn key_up<'a>(&'a self, e: &'a openaction::KeyEvent) -> crate::actions::Fut<'a> {
        let context = e.context.clone();
        tokio::spawn(async move { key_up(&context).await });
        crate::actions::done()
    }
    fn to_plugin<'a>(&'a self, e: &'a openaction::SendToPluginEvent) -> crate::actions::Fut<'a> {
        if e.payload.get("request").and_then(|r| r.as_str()) == Some("tree") {
            let context = e.context.clone();
            tokio::spawn(async move { send_tree(&context).await });
        }
        crate::actions::done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dibujos() {
        let now = render_now("aviones ›", "F-18", "● activo", "#2ecc71");
        assert!(now.contains("F-18") && now.contains("aviones ›"));
        let go = render_go(None, "aviones", "▸ entrar (2)", true, "#ff9f1a");
        assert!(go.contains("#ff9f1a") && go.contains("entrar"));
        assert!(render_now("", "<x>", "", "#fff").contains("&lt;x&gt;"));
        assert_eq!(clip("abcdefghijklmn", 5), "abcd…");
    }
}

/// Muestras de las teclas: PRISMA_SAMPLES_OUT=carpeta cargo test muestras_teclas -- --ignored
#[cfg(test)]
#[test]
#[ignore]
fn muestras_teclas() {
    let out = std::env::var("PRISMA_SAMPLES_OUT").unwrap();
    let s = [
        ("go-nombre", render_go(None, "aviones", "▸ entrar (2)", false, "#2ecc71")),
        ("go-activo", render_go(None, "F-18", "", true, "#2ecc71")),
        ("now-raiz", render_now("raíz", "Default", "● activo", "#2ecc71")),
        ("now-sub", render_now("aviones ›", "Cessna", "elegido", "#bbb")),
    ];
    for (n, svg) in s {
        std::fs::write(format!("{out}/{n}.svg"), svg).unwrap();
    }
}
