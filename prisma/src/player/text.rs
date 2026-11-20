//! Medir y recortar textos para que quepan, y deslizar los que no caben

// ------------------------------------------------------------ medir texto --

/// Ancho aproximado de un texto en el lienzo de 144 (letra sin serifa normal
/// de Windows). No hace falta exactitud: solo saber si cabe.
pub(super) fn text_width(s: &str, size: f32, bold: bool) -> f32 {
    let em: f32 = s
        .chars()
        .map(|c| match c {
            'i' | 'l' | 'j' | '.' | ',' | '\'' | '!' | '|' | ':' | ';' | 'í' | 'ì' | 'ï' => 0.26,
            ' ' | 'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '-' => 0.33,
            'm' | 'w' | 'M' | 'W' | '…' => 0.9,
            c if c.is_ascii_digit() => 0.55,
            c if c.is_uppercase() => 0.72,
            _ => 0.55,
        })
        .sum();
    // Con margen: más vale recortar un poco antes que salirse de la tecla
    em * size * if bold { 1.08 } else { 1.02 }
}

/// Recorta con "…" hasta que quepa en `max` de ancho
pub fn fit(s: &str, size: f32, bold: bool, max: f32) -> String {
    let s = s.trim();
    if text_width(s, size, bold) <= max {
        return s.to_string();
    }
    let mut out: Vec<char> = s.chars().collect();
    while !out.is_empty() && text_width(&(out.iter().collect::<String>() + "…"), size, bold) > max {
        out.pop();
    }
    out.iter().collect::<String>().trim_end().to_string() + "…"
}

/// Reparte el título en dos líneas si cabe así; si no, None
pub(super) fn two_lines(s: &str, size: f32, max: f32) -> Option<(String, String)> {
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    if text_width(s, size, true) <= max {
        return Some((s.to_string(), String::new()));
    }
    for cut in 1..words.len() {
        let a = words[..cut].join(" ");
        let b = words[cut..].join(" ");
        if text_width(&a, size, true) <= max && text_width(&b, size, true) <= max {
            return Some((a, b));
        }
    }
    None
}

/// Velocidad del deslizamiento y espera en cada extremo
pub(super) const SPEED: f32 = 40.0;
pub(super) const HOLD: f32 = 1.2;
/// Cuánto se ha deslizado en el segundo `t` un texto de `width` de ancho que
/// se ve en un hueco de `room`: espera, va hasta el final, espera y vuelve
pub(super) fn marquee_offset(width: f32, room: f32, t: f32) -> f32 {
    let travel = (width - room + 8.0).max(0.0);
    let scroll = travel / SPEED;
    let t = t % (HOLD * 2.0 + scroll);
    if t < HOLD { 0.0 } else if t < HOLD + scroll { (t - HOLD) * SPEED } else { travel }
}
