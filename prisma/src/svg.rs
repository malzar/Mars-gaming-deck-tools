//! Piezas para dibujar en SVG lo que se ve en las teclas (144×144) y en las
//! pantallas de las ruedas del MSD-PRO (200×100). Las usan todas las acciones.

use base64::Engine;

/// Lado de una tecla
pub const KEY: f32 = 144.0;
/// Pantalla de una rueda
pub const DIAL_W: f32 = 200.0;
pub const DIAL_H: f32 = 100.0;

/// Gris de los textos secundarios: claro, para que se lea bien en el deck
pub const MUTED: &str = "#b4bccb";
/// Azul de Prisma
pub const ACCENT: &str = "#3fa9f5";

/// Escapa un texto para meterlo en un SVG
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Recorta un texto largo con "…"
pub fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max { s.to_string() } else { s.chars().take(max.saturating_sub(1)).collect::<String>() + "…" }
}

/// Texto centrado en (x, y)
pub fn text(x: f32, y: f32, size: f32, color: &str, bold: bool, s: &str) -> String {
    format!(
        r##"<text x="{x}" y="{y}" font-family="sans-serif" font-size="{size}" font-weight="{w}" fill="{color}" text-anchor="middle">{t}</text>"##,
        w = if bold { "bold" } else { "normal" },
        t = esc(s)
    )
}

/// SVG completo de una tecla o de una rueda, con fondo negro
pub fn wrap(dial: bool, body: &str) -> String {
    let (w, h) = if dial { (DIAL_W, DIAL_H) } else { (KEY, KEY) };
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}"><rect width="{w}" height="{h}" fill="#000"/>{body}</svg>"##)
}

/// El SVG como imagen para `setImage`
pub fn data_url(svg: &str) -> String {
    format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(svg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piezas() {
        assert_eq!(esc(r#"<a & "b">"#), "&lt;a &amp; &quot;b&quot;&gt;");
        assert_eq!(clip("  Bohemian Rhapsody ", 8), "Bohemia…");
        assert_eq!(clip("Queen", 8), "Queen");
        assert!(wrap(true, "").contains(r#"viewBox="0 0 200 100""#));
        assert!(data_url("<svg/>").starts_with("data:image/svg+xml;base64,"));
    }
}
