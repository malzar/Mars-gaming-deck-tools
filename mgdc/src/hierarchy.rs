//! Subperfiles: qué perfil va dentro de cuál y en qué orden.
//!
//! Para OpenDeck un subperfil es un perfil normal (su fichero va con los
//! demás). La jerarquía solo la conoce Prisma y se guarda en
//! %APPDATA%\prisma\perfiles.json, en la lista `profiles` de cada deck:
//! `{ id, nombre, padre, orden }`. Un perfil sin entrada, o cuyo padre ya no
//! existe, es raíz.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Nombre del perfil en OpenDeck (su fichero)
    pub id: String,
    /// Nombre que se enseña
    pub nombre: String,
    /// Perfil dentro del que va (None = raíz)
    pub padre: Option<String>,
    /// Posición entre los de su mismo padre
    pub orden: u32,
}

/// El árbol con los perfiles que existen: los guardados (sin los que ya no
/// están) y los demás como raíz al final, en el orden de `legacy` (el orden de
/// antes de los subperfiles) y luego en el de `existing`. Corrige los padres
/// que no existen o que harían un bucle y numera el orden de 0 en adelante.
pub fn resolve(saved: &[Node], legacy: &[String], existing: &[String]) -> Vec<Node> {
    let pos = |id: &str| {
        legacy
            .iter()
            .position(|l| l == id)
            .unwrap_or(legacy.len() + existing.iter().position(|e| e == id).unwrap_or(usize::MAX / 2))
    };
    let mut nodes: Vec<Node> = vec![];
    for n in saved {
        if existing.contains(&n.id) && !nodes.iter().any(|x| x.id == n.id) {
            nodes.push(n.clone());
        }
    }
    let base = nodes.iter().map(|n| n.orden).max().map_or(0, |m| m + 1);
    for id in existing {
        if !nodes.iter().any(|x| x.id == *id) {
            nodes.push(Node { id: id.clone(), nombre: id.clone(), padre: None, orden: base + pos(id) as u32 });
        }
    }
    // Padres que no existen o bucles: a la raíz
    let ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    for n in nodes.iter_mut() {
        if n.padre.as_ref().is_some_and(|p| !ids.contains(p) || *p == n.id) {
            n.padre = None;
        }
    }
    for i in 0..nodes.len() {
        let mut seen = vec![nodes[i].id.clone()];
        let mut at = nodes[i].padre.clone();
        while let Some(p) = at {
            if seen.contains(&p) {
                nodes[i].padre = None;
                break;
            }
            seen.push(p.clone());
            at = nodes.iter().find(|n| n.id == p).and_then(|n| n.padre.clone());
        }
    }
    // Orden limpio dentro de cada padre
    let mut parents: Vec<Option<String>> = nodes.iter().map(|n| n.padre.clone()).collect();
    parents.sort();
    parents.dedup();
    for p in parents {
        let mut group: Vec<usize> = (0..nodes.len()).filter(|i| nodes[*i].padre == p).collect();
        group.sort_by_key(|i| (nodes[*i].orden, pos(&nodes[*i].id)));
        for (k, i) in group.into_iter().enumerate() {
            nodes[i].orden = k as u32;
        }
    }
    nodes
}

/// Hijos de `padre` (None = raíz), en orden
pub fn children(tree: &[Node], padre: Option<&str>) -> Vec<String> {
    let mut v: Vec<&Node> = tree.iter().filter(|n| n.padre.as_deref() == padre).collect();
    v.sort_by_key(|n| n.orden);
    v.into_iter().map(|n| n.id.clone()).collect()
}

pub fn parent_of(tree: &[Node], id: &str) -> Option<String> {
    tree.iter().find(|n| n.id == id).and_then(|n| n.padre.clone())
}

pub fn name_of(tree: &[Node], id: &str) -> String {
    tree.iter().find(|n| n.id == id).map_or_else(|| id.to_string(), |n| n.nombre.clone())
}

/// Todos, en orden de lista (cada padre seguido de sus hijos), con su nivel
pub fn flatten(tree: &[Node]) -> Vec<(String, usize)> {
    fn walk(tree: &[Node], padre: Option<&str>, depth: usize, out: &mut Vec<(String, usize)>) {
        for id in children(tree, padre) {
            out.push((id.clone(), depth));
            walk(tree, Some(&id), depth + 1, out);
        }
    }
    let mut out = vec![];
    walk(tree, None, 0, &mut out);
    out
}

/// Si `id` está dentro de `ancestor` (a cualquier profundidad)
pub fn is_inside(tree: &[Node], id: &str, ancestor: &str) -> bool {
    let mut at = parent_of(tree, id);
    let mut guard = 0;
    while let Some(p) = at {
        if p == ancestor {
            return true;
        }
        guard += 1;
        if guard > tree.len() {
            break;
        }
        at = parent_of(tree, &p);
    }
    false
}

/// Pone `ids` (hijos de `padre`) en ese orden
pub fn reorder(tree: &mut [Node], padre: Option<&str>, ids: &[String]) {
    for n in tree.iter_mut().filter(|n| n.padre.as_deref() == padre) {
        if let Some(k) = ids.iter().position(|i| *i == n.id) {
            n.orden = k as u32;
        }
    }
}

/// Mete `id` dentro de `padre`, al final
pub fn set_parent(tree: &mut Vec<Node>, id: &str, padre: Option<&str>) -> Result<(), String> {
    if let Some(p) = padre
        && (p == id || is_inside(tree, p, id))
    {
        return Err(crate::i18n::t("error.profile_inside_itself").into());
    }
    let next = tree.iter().filter(|n| n.padre.as_deref() == padre && n.id != id).map(|n| n.orden + 1).max().unwrap_or(0);
    match tree.iter_mut().find(|n| n.id == id) {
        Some(n) => {
            n.padre = padre.map(String::from);
            n.orden = next;
        }
        None => tree.push(Node { id: id.into(), nombre: id.into(), padre: padre.map(String::from), orden: next }),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    fn n(id: &str, padre: Option<&str>, orden: u32) -> Node {
        Node { id: id.into(), nombre: id.into(), padre: padre.map(String::from), orden }
    }

    #[test]
    fn sin_jerarquia_todo_es_raiz() {
        let t = resolve(&[], &s(&["coches"]), &s(&["Default", "aviones", "coches"]));
        assert_eq!(children(&t, None), s(&["coches", "Default", "aviones"]));
    }

    #[test]
    fn raiz_y_subperfiles() {
        let saved = vec![n("Default", None, 0), n("aviones", None, 1), n("F-18", Some("aviones"), 0), n("Cessna", Some("aviones"), 1)];
        let t = resolve(&saved, &[], &s(&["Default", "aviones", "F-18", "Cessna", "coches"]));
        assert_eq!(children(&t, None), s(&["Default", "aviones", "coches"]));
        assert_eq!(children(&t, Some("aviones")), s(&["F-18", "Cessna"]));
        assert_eq!(
            flatten(&t),
            vec![("Default".into(), 0), ("aviones".into(), 0), ("F-18".into(), 1), ("Cessna".into(), 1), ("coches".into(), 0)]
        );
    }

    #[test]
    fn padres_perdidos_y_bucles() {
        let saved = vec![n("a", Some("b"), 0), n("b", Some("a"), 0), n("c", Some("borrado"), 0)];
        let t = resolve(&saved, &[], &s(&["a", "b", "c"]));
        assert!(parent_of(&t, "c").is_none());
        // a y b no pueden estar uno dentro del otro a la vez
        assert!(parent_of(&t, "a").is_none() || parent_of(&t, "b").is_none());
    }

    /// Lo que hace arrastrar en el panel: un subperfil pasa a perfil y al revés
    #[test]
    fn subperfil_a_perfil_y_al_reves() {
        let saved = vec![n("Default", None, 0), n("prueba 2", None, 1), n("CPU", None, 2), n("ETHERNET", Some("CPU"), 0), n("GPU", Some("CPU"), 1)];
        let ids = s(&["Default", "prueba 2", "CPU", "ETHERNET", "GPU"]);
        let mut t = resolve(&saved, &[], &ids);
        // ETHERNET encima de «prueba 2» (a la raíz)
        set_parent(&mut t, "ETHERNET", None).unwrap();
        reorder(&mut t, None, &s(&["Default", "ETHERNET", "prueba 2", "CPU"]));
        assert_eq!(children(&t, None), s(&["Default", "ETHERNET", "prueba 2", "CPU"]));
        assert_eq!(children(&t, Some("CPU")), s(&["GPU"]));
        // «prueba 2» dentro de CPU, encima de GPU
        set_parent(&mut t, "prueba 2", Some("CPU")).unwrap();
        reorder(&mut t, Some("CPU"), &s(&["prueba 2", "GPU"]));
        assert_eq!(children(&t, Some("CPU")), s(&["prueba 2", "GPU"]));
        // CPU no puede ir dentro de su subperfil
        assert!(set_parent(&mut t, "CPU", Some("GPU")).is_err());
        assert_eq!(parent_of(&t, "CPU"), None);
    }

    #[test]
    fn mover_y_ordenar() {
        let mut t = resolve(&[], &[], &s(&["A", "B", "B1", "B2"]));
        set_parent(&mut t, "B1", Some("B")).unwrap();
        set_parent(&mut t, "B2", Some("B")).unwrap();
        assert!(set_parent(&mut t, "B", Some("B1")).is_err());
        reorder(&mut t, Some("B"), &s(&["B2", "B1"]));
        assert_eq!(children(&t, Some("B")), s(&["B2", "B1"]));
        assert!(is_inside(&t, "B2", "B"));
    }
}
