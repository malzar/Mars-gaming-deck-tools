//! Estela de las animaciones por pasos: lo que deja de estar encendido va
//! bajando de brillo hasta apagarse. O, en modo fijo, se queda encendido y al
//! final se apaga todo de golpe.

use super::patterns::Step;

/// Estela: brillo de una tecla en cada paso desde que deja de estar encendida.
/// Con `n` pasos baja en línea recta hasta apagarse; con 3, por ejemplo:
/// encendida → 75 % → 50 % → 25 % → apagada.
pub(super) fn trail_levels(n: u8) -> Vec<f32> {
    let n = n.min(3) as usize;
    (0..=n).map(|i| 1.0 - i as f32 / (n + 1) as f32).collect()
}

/// Brillo de una tecla en un paso: `Some(i)` = `levels[i]`, `None` = apagada
pub(super) type Level = Option<usize>;

/// Convierte los pasos (teclas encendidas) en cambios con estela: lo que deja
/// de estar encendido va bajando un escalón de `levels` por paso hasta
/// apagarse. Al final se añaden pasos para que también se apague la última estela.
pub(super) fn trail_frames(steps: &[Step], levels: usize) -> Vec<Vec<(u8, Level)>> {
    use std::collections::HashMap;
    let mut state: HashMap<u8, usize> = HashMap::new();
    let mut frames = vec![];
    let empty: Step = vec![];
    for step in steps.iter().chain(std::iter::repeat_n(&empty, levels)) {
        let mut changes = vec![];
        let mut keys: Vec<u8> = state.keys().copied().chain(step.iter().copied()).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            let before = state.get(&k).copied();
            let after = if step.contains(&k) {
                Some(0)
            } else {
                before.map(|i| i + 1).filter(|&i| i < levels)
            };
            if after != before {
                changes.push((k, after));
            }
            match after {
                Some(i) => { state.insert(k, i); }
                None => { state.remove(&k); }
            }
        }
        if !changes.is_empty() {
            frames.push(changes);
        }
    }
    frames
}

/// Modo fijo: las teclas se encienden según los pasos, se quedan encendidas
/// y al final se apagan todas de golpe
pub(super) fn fixed_frames(steps: &[Step]) -> Vec<Vec<(u8, Level)>> {
    let mut lit: Vec<u8> = vec![];
    let mut frames = vec![];
    for step in steps {
        let on: Vec<(u8, Level)> = step.iter().filter(|k| !lit.contains(k)).map(|&k| (k, Some(0))).collect();
        lit.extend(on.iter().map(|&(k, _)| k));
        if !on.is_empty() {
            frames.push(on);
        }
    }
    if !lit.is_empty() {
        frames.push(lit.iter().map(|&k| (k, None)).collect());
    }
    frames
}
