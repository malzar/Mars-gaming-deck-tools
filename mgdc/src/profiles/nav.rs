//! Navegación por niveles.
//!
//! La rueda recorre los perfiles del mismo nivel que el activo (los que tienen
//! el mismo padre, ver hierarchy.rs), con vuelta al principio, y nunca sube ni
//! baja: eso lo hace la tecla «Entrar / cambiar». El estado es por deck, así lo
//! comparten la rueda y las teclas («Perfil actual» enseña el elegido).
//! Si el nivel está vacío, o en la raíz solo hay un perfil, la rueda recorre
//! todos los perfiles como antes (lista completa en el orden del panel).

use std::{collections::HashMap, sync::LazyLock};

use tokio::sync::Mutex;

use super::{
    draw::redraw_device,
    slots::{Slot, first_virtual, row_of, switch_device},
    store::{active_profile, list_profiles, read_profile},
};

#[derive(Default, Clone)]
struct Nav {
    /// Nivel que recorre la rueda: los hijos de este perfil (None = raíz)
    level: Option<String>,
    /// Perfil elegido con la rueda
    selected: Option<String>,
    /// Perfil activo la última vez que se miró
    seen: Option<String>,
    /// Cambio que ha pedido Prisma (al llegar no se toca el nivel)
    expect: Option<String>,
}
static NAV: LazyLock<Mutex<HashMap<String, Nav>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Lo que hay ahora en un deck
#[derive(Clone, Default)]
pub struct View {
    /// Perfiles que recorre la rueda, en orden
    pub list: Vec<String>,
    /// Posición del elegido en `list`
    pub at: usize,
    pub selected: Option<String>,
    pub active: Option<String>,
    /// Nivel (padre de los de `list`; None = raíz)
    pub level: Option<String>,
    /// La rueda recorre todos (no hay niveles que aplicar)
    pub flat: bool,
    pub tree: Vec<crate::hierarchy::Node>,
    /// Perfiles con «Switch Profile» en las teclas virtuales del activo
    pub available: Vec<String>,
}

impl View {
    /// Subperfiles de `id` a los que se puede ir
    pub fn kids(&self, id: &str) -> Vec<String> {
        crate::hierarchy::children(&self.tree, Some(id)).into_iter().filter(|k| self.available.contains(k)).collect()
    }
    pub fn name(&self, id: &str) -> String {
        crate::hierarchy::name_of(&self.tree, id)
    }
    /// Texto del nivel: "RAÍZ", "AVIONES ›"…
    pub fn level_label(&self) -> String {
        match (&self.level, self.flat) {
            (Some(l), false) => format!("{} ›", self.name(l)),
            _ => crate::i18n::t("profiles.title").into(),
        }
    }
}

/// Un perfil ha cambiado de nombre: lo que tenía elegido la rueda lo sigue
pub async fn rename(device: &str, old: &str, new: &str) {
    if let Some(n) = NAV.lock().await.get_mut(device) {
        for f in [&mut n.level, &mut n.selected, &mut n.seen, &mut n.expect] {
            if f.as_deref() == Some(old) {
                *f = Some(new.to_string());
            }
        }
    }
}

/// Mira el perfil activo, el árbol y lo elegido, y lo deja todo cuadrado
pub async fn view(device: &str) -> View {
    let Some((first, n)) = first_virtual(device).await else { return View::default() };
    let d = device.to_string();
    let read = tokio::task::spawn_blocking(move || {
        let existing = list_profiles(&d);
        let active = active_profile(&d);
        let slots = read_profile(&d, active.as_deref().unwrap_or("Default"))
            .map(|v| row_of(&v, first as usize, n as usize))
            .unwrap_or_default();
        let tree = crate::profile_icons::tree(&d, &existing);
        (active, slots, tree)
    })
    .await;
    let Ok((active, slots, tree)) = read else { return View::default() };
    let mut available: Vec<String> = vec![];
    for s in &slots {
        if let Slot::Switch(t) = s
            && !available.contains(t)
        {
            available.push(t.clone());
        }
    }
    let mut navs = NAV.lock().await;
    let nav = navs.entry(device.to_string()).or_default();
    let (list, flat) = settle(nav, active.as_deref(), &tree, &available);
    let selected = nav.selected.clone();
    let at = selected.as_ref().and_then(|s| list.iter().position(|x| x == s)).unwrap_or(0);
    View { list, at, selected, active, level: nav.level.clone(), flat, tree, available }
}

/// Pone al día el estado con el perfil activo y devuelve lo que recorre la
/// rueda y si es la lista completa (cuando no hay niveles que aplicar)
fn settle(nav: &mut Nav, active: Option<&str>, tree: &[crate::hierarchy::Node], available: &[String]) -> (Vec<String>, bool) {
    if nav.seen.as_deref() != active {
        if nav.expect.is_some() && nav.expect.as_deref() == active {
            nav.expect = None;
        } else {
            // Cambio de fuera (OpenDeck, una tecla, los enlaces): al nivel del activo
            nav.level = active.and_then(|a| crate::hierarchy::parent_of(tree, a));
            nav.selected = active.map(String::from);
            nav.expect = None;
        }
        nav.seen = active.map(String::from);
    }
    let mut list: Vec<String> =
        crate::hierarchy::children(tree, nav.level.as_deref()).into_iter().filter(|p| available.contains(p)).collect();
    let flat = list.is_empty() || (nav.level.is_none() && list.len() <= 1);
    if flat {
        list = crate::hierarchy::flatten(tree).into_iter().map(|(id, _)| id).filter(|p| available.contains(p)).collect();
    }
    if !nav.selected.as_ref().is_some_and(|s| list.contains(s)) {
        nav.selected = list.first().cloned();
    }
    (list, flat)
}

/// Lo que hace ir a `target` en el estado (el cambio de perfil va aparte)
fn nav_go(nav: &mut Nav, v: &View, target: &str, enter: bool) {
    let kids = v.kids(target);
    if enter && !kids.is_empty() {
        nav.level = Some(target.to_string());
        nav.selected = kids.first().cloned();
    } else {
        if !v.list.iter().any(|p| p == target) || v.flat {
            nav.level = crate::hierarchy::parent_of(&v.tree, target);
        }
        nav.selected = Some(target.to_string());
    }
    if v.active.as_deref() != Some(target) {
        nav.expect = Some(target.to_string());
    }
}

/// Subir un nivel en el estado: el padre queda elegido
fn nav_up(nav: &mut Nav, v: &View) {
    if let Some(l) = v.level.clone() {
        nav.level = crate::hierarchy::parent_of(&v.tree, &l);
        nav.selected = Some(l);
    }
}

/// El siguiente de `list` a `ticks` pasos de `at`, con vuelta al principio
fn wrap(list: &[String], at: usize, ticks: i16) -> Option<String> {
    if list.is_empty() {
        return None;
    }
    let n = list.len() as i32;
    Some(list[(((at as i32 + ticks as i32) % n) + n) as usize % n as usize].clone())
}

/// Va a un perfil. Con `enter`, si tiene subperfiles la rueda pasa a ellos.
pub async fn go(device: &str, target: &str, enter: bool) {
    let v = view(device).await;
    nav_go(NAV.lock().await.entry(device.to_string()).or_default(), &v, target, enter);
    if v.active.as_deref() != Some(target) {
        log::info!("Perfiles: {device} → «{target}»");
        switch_device(device, target).await;
    }
    redraw_device(device).await;
}

/// Sube un nivel (sin cambiar de perfil): el padre queda elegido
pub async fn up(device: &str) {
    let v = view(device).await;
    nav_up(NAV.lock().await.entry(device.to_string()).or_default(), &v);
    redraw_device(device).await;
}

/// Mueve lo elegido `ticks` pasos dentro del nivel (con vuelta al principio)
pub(super) async fn step(device: &str, ticks: i16) -> Option<String> {
    let v = view(device).await;
    let next = wrap(&v.list, v.at, ticks)?;
    NAV.lock().await.entry(device.to_string()).or_default().selected = Some(next.clone());
    Some(next)
}

/// Cambia al perfil elegido con la rueda (sin cambiar de nivel)
pub(super) async fn choose(device: &str) {
    if let Some(target) = view(device).await.selected {
        go(device, &target, false).await;
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// El flujo del diseño: Default (raíz) → giro → aviones → entrar →
    /// subperfiles de aviones → giro con vuelta → nunca cambia de nivel
    #[test]
    fn flujo_por_niveles() {
        use crate::hierarchy::Node;
        let n = |id: &str, padre: Option<&str>, orden: u32| Node { id: id.into(), nombre: id.into(), padre: padre.map(String::from), orden };
        let tree = vec![n("Default", None, 0), n("aviones", None, 1), n("F-18", Some("aviones"), 0), n("Cessna", Some("aviones"), 1), n("coches", None, 2)];
        let available: Vec<String> = ["Default", "aviones", "F-18", "Cessna", "coches"].iter().map(|s| s.to_string()).collect();
        let mut nav = Nav::default();
        let mut active = "Default".to_string();
        let mk = |nav: &mut Nav, active: &str| {
            let (list, flat) = settle(nav, Some(active), &tree, &available);
            let at = nav.selected.as_ref().and_then(|s| list.iter().position(|x| x == s)).unwrap_or(0);
            View { list, at, selected: nav.selected.clone(), active: Some(active.into()), level: nav.level.clone(), flat, tree: tree.clone(), available: available.clone() }
        };
        // 1 · En Default (raíz): la rueda recorre la raíz
        let v = mk(&mut nav, &active);
        assert_eq!(v.list, ["Default", "aviones", "coches"]);
        assert_eq!(v.selected.as_deref(), Some("Default"));
        assert!(!v.flat);
        // 2 · Giro → aviones (el «Perfil actual» enseña el elegido)
        nav.selected = wrap(&v.list, v.at, 1);
        let v = mk(&mut nav, &active);
        assert_eq!(v.selected.as_deref(), Some("aviones"));
        assert_eq!(v.kids("aviones"), ["F-18", "Cessna"]);
        // Vuelta al principio hacia atrás: desde Default, -1 → coches
        assert_eq!(wrap(&v.list, 0, -1).as_deref(), Some("coches"));
        // 3 · Botón de acción: cambia a aviones y entra
        nav_go(&mut nav, &v, "aviones", true);
        active = "aviones".into();
        let v = mk(&mut nav, &active);
        assert_eq!(v.list, ["F-18", "Cessna"]);
        assert_eq!(v.level.as_deref(), Some("aviones"));
        assert_eq!(v.level_label(), "aviones ›");
        // 4 · Giros: F-18 → Cessna → F-18 (vuelta), sin salir de aviones
        nav.selected = wrap(&v.list, v.at, 1);
        let v = mk(&mut nav, &active);
        assert_eq!(v.selected.as_deref(), Some("Cessna"));
        nav.selected = wrap(&v.list, v.at, 1);
        let v = mk(&mut nav, &active);
        assert_eq!(v.selected.as_deref(), Some("F-18"));
        assert_eq!(v.level.as_deref(), Some("aviones"));
        for t in [1, 3, -7, 20] {
            nav.selected = wrap(&v.list, v.at, t);
            assert_eq!(mk(&mut nav, &active).level.as_deref(), Some("aviones"));
        }
        // 5 · Pulsar el knob en Cessna: cambia a Cessna, el nivel sigue
        nav.selected = Some("Cessna".into());
        let v = mk(&mut nav, &active);
        nav_go(&mut nav, &v, "Cessna", false);
        active = "Cessna".into();
        let v = mk(&mut nav, &active);
        assert_eq!(v.level.as_deref(), Some("aviones"));
        assert_eq!(v.selected.as_deref(), Some("Cessna"));
        // Mantener el botón de acción: sube, con aviones elegido
        nav_up(&mut nav, &v);
        let v = mk(&mut nav, &active);
        assert_eq!(v.list, ["Default", "aviones", "coches"]);
        assert_eq!(v.selected.as_deref(), Some("aviones"));
        // Cambio de fuera (OpenDeck) a F-18: la rueda va al nivel de F-18
        let v = mk(&mut nav, "F-18");
        assert_eq!(v.list, ["F-18", "Cessna"]);
    }

    /// Sin jerarquía (o un nivel que no sirve), la rueda hace lo de antes:
    /// recorre todos los perfiles
    #[test]
    fn sin_niveles_lista_completa() {
        let available: Vec<String> = ["Default", "aviones", "coches"].iter().map(|s| s.to_string()).collect();
        let tree = crate::hierarchy::resolve(&[], &[], &available);
        let mut nav = Nav::default();
        let (list, flat) = settle(&mut nav, Some("aviones"), &tree, &available);
        assert_eq!(list, ["Default", "aviones", "coches"]);
        assert!(!flat || list.len() == 3);
        // Un nivel sin perfiles disponibles: lista completa
        let mut nav = Nav { level: Some("coches".into()), seen: Some("aviones".into()), ..Default::default() };
        let (list, flat) = settle(&mut nav, Some("aviones"), &tree, &available);
        assert!(flat);
        assert_eq!(list.len(), 3);
        // Solo un perfil en la raíz: lista completa
        let one: Vec<String> = vec!["Default".into(), "solo".into()];
        let mut t = crate::hierarchy::resolve(&[], &[], &one);
        crate::hierarchy::set_parent(&mut t, "solo", Some("Default")).unwrap();
        let mut nav = Nav::default();
        let (list, flat) = settle(&mut nav, Some("Default"), &t, &one);
        assert!(flat);
        assert_eq!(list, ["Default", "solo"]);
    }
}
