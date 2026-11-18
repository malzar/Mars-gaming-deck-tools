//! Los dibujos de las animaciones por pasos: qué teclas se encienden en cada
//! paso. Cada uno es una función de la rejilla del deck; [`of`] dice cuál va
//! con cada tipo de animación. Para añadir uno basta con escribirlo y
//! apuntarlo ahí.

use super::{color::Rng, screen::anim_grid};
use crate::{family::Model, settings::AnimKind};

/// Un paso de animación: las teclas (posiciones de OpenDeck) encendidas en ese momento
pub(super) type Step = Vec<u8>;

/// El dibujo de cada animación por pasos (None si no es de pasos)
pub(super) fn of(kind: &AnimKind) -> Option<fn(&Model) -> Vec<Step>> {
    Some(match kind {
        AnimKind::Chase => chase_steps,
        AnimKind::Arrow => arrow_steps,
        AnimKind::Random => random_steps,
        AnimKind::Snake => snake_steps,
        AnimKind::Bounce => bounce_steps,
        AnimKind::Meet => meet_steps,
        AnimKind::Rain => rain_steps,
        AnimKind::Windmill => windmill_steps,
        AnimKind::Cross => cross_steps,
        AnimKind::Swing => swing_steps,
        _ => return None,
    })
}

/// Lineal: 0, 1, 2… hasta la última, de una en una
pub(super) fn chase_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    (0..rows * cols).map(|k| vec![k]).collect()
}

// ----- Animaciones ligeras: como mucho 2 teclas encendidas por paso, así que
// sin estela cambian 4 teclas por paso como máximo (2 se encienden, 2 se apagan)

/// Serpiente: fila 0 de izquierda a derecha, fila 1 de derecha a izquierda…
pub(super) fn snake_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    (0..rows)
        .flat_map(|r| {
            let cs: Vec<u8> = if r % 2 == 0 { (0..cols).collect() } else { (0..cols).rev().collect() };
            cs.into_iter().map(move |c| vec![r * cols + c])
        })
        .collect()
}

/// Orden de las teclas en espiral (borde exterior en el sentido del reloj, y hacia dentro).
/// Lo usa el Molino para recorrer los anillos.
pub(super) fn spiral_order(rows: u8, cols: u8) -> Vec<u8> {
    let (mut top, mut left) = (0i32, 0i32);
    let (mut bottom, mut right) = (rows as i32 - 1, cols as i32 - 1);
    let mut v = vec![];
    while top <= bottom && left <= right {
        for c in left..=right { v.push((top, c)); }
        for r in top + 1..=bottom { v.push((r, right)); }
        if top < bottom { for c in (left..right).rev() { v.push((bottom, c)); } }
        if left < right { for r in (top + 1..bottom).rev() { v.push((r, left)); } }
        top += 1; left += 1; bottom -= 1; right -= 1;
    }
    v.into_iter().map(|(r, c)| (r * cols as i32 + c) as u8).collect()
}

/// Rebote: una tecla que va en diagonal y rebota contra los bordes. En
/// diagonal solo pisa la mitad de las teclas (como los alfiles en el ajedrez),
/// así que al terminar esa mitad sigue desde la tecla de al lado con la otra.
pub(super) fn bounce_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    let (rows, cols) = (rows as i32, cols as i32);
    let total = (rows * cols) as usize;
    let mut seen = std::collections::HashSet::new();
    let mut v = vec![];
    for (mut r, mut c) in [(0i32, 0i32), (0, 1.min(cols - 1))] {
        let (mut dr, mut dc) = (1i32, 1i32);
        let mut states = std::collections::HashSet::new();
        for _ in 0..4 * total {
            // En algunas rejillas (3×5) la diagonal se repite sin pisar todas:
            // salta a la siguiente tecla que falte de esa mitad
            if !states.insert((r, c, dr, dc)) {
                let parity = (r + c) % 2;
                if let Some(i) = (0..rows * cols).find(|i| (i / cols + i % cols) % 2 == parity && !seen.contains(&(*i as u8))) {
                    (r, c) = (i / cols, i % cols);
                }
            }
            let k = (r * cols + c) as u8;
            v.push(vec![k]);
            seen.insert(k);
            // Esta mitad ya está completa
            let parity = (r + c) % 2;
            let half = (0..rows * cols).filter(|i| (i / cols + i % cols) % 2 == parity).count();
            let done = seen.iter().filter(|&&i| (i as i32 / cols + i as i32 % cols) % 2 == parity).count();
            if done == half || seen.len() == total {
                break;
            }
            if rows > 1 && !(0..rows).contains(&(r + dr)) { dr = -dr; }
            if !(0..cols).contains(&(c + dc)) { dc = -dc; }
            if rows > 1 { r += dr; }
            c += dc;
        }
        if seen.len() == total {
            break;
        }
    }
    v
}

/// Encuentro: una tecla sale del principio y otra del final y se cruzan en el centro
pub(super) fn meet_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    let n = rows as usize * cols as usize;
    (0..n.div_ceil(2))
        .map(|i| {
            let (a, b) = (i as u8, (n - 1 - i) as u8);
            if a == b { vec![a] } else { vec![a, b] }
        })
        .collect()
}

/// Lluvia: gotas que caen por columnas al azar; cada gota empieza cuando la
/// anterior va por la penúltima fila, así nunca hay más de 2 a la vez
pub(super) fn rain_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    let mut order: Vec<u8> = (0..cols).collect();
    let mut rng = Rng::new();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i + 1));
    }
    let gap = (rows as usize).saturating_sub(1).max(1);
    let total = gap * (order.len() - 1) + rows as usize;
    (0..total)
        .map(|t| {
            order
                .iter()
                .enumerate()
                .filter_map(|(d, &c)| {
                    let row = t as i64 - (d * gap) as i64;
                    (0..rows as i64).contains(&row).then(|| row as u8 * cols + c)
                })
                .collect()
        })
        .collect()
}

/// Anillos de la cuadrícula, de fuera hacia dentro, cada uno en el sentido del reloj
pub(super) fn rings(rows: u8, cols: u8) -> Vec<Vec<u8>> {
    let order = spiral_order(rows, cols);
    let (mut r, mut c) = (rows as usize, cols as usize);
    let mut out = vec![];
    let mut i = 0;
    while r > 0 && c > 0 && i < order.len() {
        let len = if r == 1 || c == 1 { r * c } else { 2 * (r + c) - 4 };
        out.push(order[i..(i + len).min(order.len())].to_vec());
        i += len;
        r = r.saturating_sub(2);
        c = c.saturating_sub(2);
    }
    out
}

/// Molino: dos teclas opuestas dan una vuelta entera por el borde y luego
/// otra por cada anillo de dentro, así pasa por todas las teclas
pub(super) fn windmill_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    let mut v = vec![];
    for ring in rings(rows, cols) {
        let l = ring.len();
        if l == 1 {
            v.push(vec![ring[0]]);
            continue;
        }
        for i in 0..l {
            let (a, b) = (ring[i], ring[(i + l / 2) % l]);
            v.push(if a == b { vec![a] } else { vec![a, b] });
        }
    }
    v
}

/// Cruce: dos teclas en diagonal, una desde arriba y otra desde abajo, que
/// rebotan en espejo y se cruzan en la fila del centro. A la ida pisan la
/// mitad de las teclas; a la vuelta van desfasadas una fila y pisan la otra mitad.
pub(super) fn cross_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    let last = rows as i32 - 1;
    let period = (2 * last).max(1);
    let row = |t: i32| {
        let p = t.rem_euclid(period);
        if p <= last { p } else { period - p }
    };
    let pair = |r: i32, c: i32| {
        let a = (r * cols as i32 + c) as u8;
        let b = ((last - r) * cols as i32 + c) as u8;
        if a == b { vec![a] } else { vec![a, b] }
    };
    let ida = (0..cols as i32).map(|c| pair(row(c), c));
    let vuelta = (0..cols as i32).rev().map(|c| pair(row(c + 1), c));
    ida.chain(vuelta).collect()
}

/// Vaivén: una pareja (arriba y abajo de la misma columna) va hacia la
/// derecha y vuelve por la fila del centro, dos teclas cada vez
pub(super) fn swing_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    let last = rows - 1;
    let mut v: Vec<Step> = (0..cols)
        .map(|c| if last == 0 { vec![c] } else { vec![c, last * cols + c] })
        .collect();
    for r in 1..last {
        let mut c = cols as i32 - 1;
        while c >= 0 {
            let mut step = vec![(r * cols) as i32 + c];
            if c > 0 {
                step.push((r * cols) as i32 + c - 1);
            }
            v.push(step.into_iter().map(|k| k as u8).collect());
            c -= 2;
        }
    }
    v
}

/// Flecha: la fila del centro va delante y las demás se retrasan una columna
/// por cada fila de distancia. En el MSD-ONE (3×6): 6 → 7,0,12 → 8,1,13 → … → 5,17.
pub(super) fn arrow_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    let mid = (rows as i32 - 1) / 2;
    let max_lag = (0..rows as i32).map(|r| (r - mid).abs()).max().unwrap_or(0);
    (0..cols as i32 + max_lag)
        .map(|k| {
            (0..rows as i32)
                .filter_map(|r| {
                    let c = k - (r - mid).abs();
                    (0..cols as i32).contains(&c).then(|| (r * cols as i32 + c) as u8)
                })
                .collect()
        })
        .collect()
}

/// Aleatorio: todas las teclas en orden al azar, de una en una, sin repetir
pub(super) fn random_steps(model: &Model) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    let mut keys: Vec<u8> = (0..rows * cols).collect();
    let mut rng = Rng::new();
    for i in (1..keys.len()).rev() {
        keys.swap(i, rng.below(i + 1));
    }
    keys.into_iter().map(|k| vec![k]).collect()
}

/// Invertir: lo que iba de arriba abajo va de abajo arriba, y lo que iba de
/// izquierda a derecha va de derecha a izquierda (la animación girada 180°)
pub(super) fn mirror(model: &Model, steps: Vec<Step>) -> Vec<Step> {
    let (rows, cols) = anim_grid(model);
    steps
        .into_iter()
        .map(|s| {
            s.into_iter()
                .map(|k| (rows - 1 - k / cols) * cols + (cols - 1 - k % cols))
                .collect()
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::trail::{fixed_frames, trail_frames, trail_levels};

    #[test]
    fn flecha_y_filas_en_el_msd_one() {
        let one = Model::from_vid_pid(0x0b00, 0x1000).unwrap();
        let a = arrow_steps(&one);
        assert_eq!(a[0], vec![6]);
        assert_eq!(a[1], vec![0, 7, 12]);
        assert_eq!(a[2], vec![1, 8, 13]);
        assert_eq!(a.last().unwrap(), &vec![5, 17]);
        assert_eq!(a.len(), 7);
        // Estela de 3 pasos en la flecha: el 6 pasa por 75 %, 50 %, 25 % y se apaga
        assert_eq!(trail_levels(3), vec![1.0, 0.75, 0.5, 0.25]);
        assert_eq!(trail_levels(1), vec![1.0, 0.5]);
        assert_eq!(trail_levels(10).len(), 4);
        assert_eq!(trail_levels(0), vec![1.0]);
        // Las 6 animaciones ligeras: sin estela, como mucho 4 teclas cambian por paso
        for steps in [snake_steps(&one), bounce_steps(&one), meet_steps(&one), rain_steps(&one), windmill_steps(&one), cross_steps(&one), swing_steps(&one)] {
            let worst = trail_frames(&steps, 1).iter().map(|f| f.len()).max().unwrap();
            assert!(worst <= 4, "{worst} cambios en {steps:?}");
        }
        assert_eq!(spiral_order(3, 6), vec![0, 1, 2, 3, 4, 5, 11, 17, 16, 15, 14, 13, 12, 6, 7, 8, 9, 10]);
        assert_eq!(snake_steps(&one)[6], vec![11]);
        assert_eq!(meet_steps(&one)[0], vec![0, 17]);
        // Molino, Vaivén, Rebote y Cruce pasan por todas las teclas
        let pro = Model::from_vid_pid(0x0b00, 0x1003).unwrap();
        for steps in [
            windmill_steps(&one), swing_steps(&one), bounce_steps(&one), cross_steps(&one),
            windmill_steps(&pro), swing_steps(&pro), bounce_steps(&pro), cross_steps(&pro),
        ] {
            let mut all: Vec<u8> = steps.into_iter().flatten().collect();
            all.sort();
            all.dedup();
            assert_eq!(all.len(), all.last().map_or(0, |&m| m as usize + 1), "{all:?}");
        }
        assert_eq!(rings(3, 6)[1], vec![7, 8, 9, 10]);
        assert_eq!(cross_steps(&one)[0], vec![0, 12]);
        assert_eq!(cross_steps(&one)[1], vec![7]);
        assert_eq!(cross_steps(&one)[6], vec![17, 5]);
        assert_eq!(swing_steps(&one)[0], vec![0, 12]);
        // Invertir: la flecha va hacia la izquierda (empieza en el 11, luego 17, 10 y 5)
        let inv = mirror(&one, arrow_steps(&one));
        assert_eq!(inv[0], vec![11]);
        assert_eq!(inv[1], vec![17, 10, 5]);
        assert_eq!(inv.last().unwrap(), &vec![12, 0]);
        // La lineal invertida va del 17 al 0
        let lin = mirror(&one, chase_steps(&one));
        assert_eq!(lin[0], vec![17]);
        assert_eq!(lin.last().unwrap(), &vec![0]);
        // Ajustes guardados con la opción antigua "fixed_off" se siguen leyendo
        let parsed: crate::settings::Animation =
            serde_json::from_str(r#"{"kind":"arrow","fixed":true,"fixed_off":"same","reverse":true}"#).unwrap();
        assert!(parsed.fixed && parsed.reverse);
        assert_eq!(swing_steps(&one)[6], vec![11, 10]);
        let pro = Model::from_vid_pid(0x0b00, 0x1003).unwrap();
        for steps in [snake_steps(&pro), bounce_steps(&pro), meet_steps(&pro), rain_steps(&pro), windmill_steps(&pro), cross_steps(&pro), swing_steps(&pro)] {
            assert!(steps.iter().flatten().all(|&k| k < 15));
            // y usan la fila de las pantallas de las ruedas
            assert!(steps.iter().flatten().any(|&k| k >= 10));
        }
        // Fijo: se encienden, se quedan y al final se apagan todas de golpe
        let fx = fixed_frames(&a);
        assert_eq!(fx.len(), a.len() + 1);
        assert_eq!(fx.last().unwrap().len(), 18);
        let f = trail_frames(&a, 4);
        assert!(f[1].contains(&(6, Some(1))) && f[1].contains(&(7, Some(0))));
        assert!(f[2].contains(&(6, Some(2))) && f[2].contains(&(7, Some(1))) && f[2].contains(&(8, Some(0))));
        assert!(f[3].contains(&(6, Some(3))));
        assert!(f[4].contains(&(6, None)));
        // Al final: 5 y 17 bajan y se apagan
        assert_eq!(f.len(), a.len() + 4);
        assert!(f.last().unwrap().iter().all(|&(_, l)| l.is_none()));
        // Lineal: 0 → 75 % → 50 % → 25 % → apagado
        let l = trail_frames(&chase_steps(&one), 4);
        assert_eq!(l[0], vec![(0, Some(0))]);
        assert_eq!(l[1], vec![(0, Some(1)), (1, Some(0))]);
        assert_eq!(l[4], vec![(0, None), (1, Some(3)), (2, Some(2)), (3, Some(1)), (4, Some(0))]);
        let mut all: Vec<u8> = random_steps(&one).into_iter().flatten().collect();
        all.sort();
        assert_eq!(all, (0..18).collect::<Vec<u8>>());
    }
}

#[cfg(test)]
#[test]
#[ignore]
fn volcar_pasos() {
    let one = Model::from_vid_pid(0x0b00, 0x1000).unwrap();
    let all = [
        ("Lineal", chase_steps(&one)), ("Flecha", arrow_steps(&one)),
        ("Aleatorio", random_steps(&one)), ("Serpiente", snake_steps(&one)),
        ("Rebote", bounce_steps(&one)), ("Encuentro", meet_steps(&one)), ("Lluvia", rain_steps(&one)),
        ("Molino", windmill_steps(&one)), ("Cruce", cross_steps(&one)), ("Vaivén", swing_steps(&one)),
    ];
    for (name, steps) in all {
        println!("{name}|{:?}", steps);
    }
}

