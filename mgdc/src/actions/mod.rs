//! Reparte los eventos de OpenDeck entre los grupos de acciones de Prisma.
//!
//! Cada grupo (audio, perfiles, hora y tiempo, temporizadores…) implementa
//! [`ActionGroup`] solo con los eventos que le interesan; el resto no hacen
//! nada. Para añadir un grupo nuevo basta con implementarlo y apuntarlo en
//! [`GROUPS`]: el reparto no cambia.

use std::{future::Future, pin::Pin};

use openaction::{
    AppearEvent, DialPressEvent, DialRotateEvent, DidReceiveSettingsEvent, EventHandlerResult, KeyEvent, OutboundEventManager,
    SendToPluginEvent,
};

/// Lo que devuelve cada evento de un grupo
pub type Fut<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Un evento que no hace nada
pub fn done<'a>() -> Fut<'a> {
    Box::pin(async {})
}

/// Un grupo de acciones de Prisma (las de un mismo tipo, con su UUID)
pub trait ActionGroup: Sync {
    /// Si la acción (su UUID) es de este grupo
    fn owns(&self, action: &str) -> bool;

    /// Aparece en el deck. Se espera a que termine antes del siguiente evento:
    /// lo que tarde se lanza aparte.
    fn appear<'a>(&'a self, _event: &'a AppearEvent, _outbound: &'a mut OutboundEventManager) -> Fut<'a> {
        done()
    }
    fn disappear<'a>(&'a self, _event: &'a AppearEvent) -> Fut<'a> {
        done()
    }
    fn settings<'a>(&'a self, _event: &'a DidReceiveSettingsEvent) -> Fut<'a> {
        done()
    }
    fn key_down<'a>(&'a self, _event: &'a KeyEvent) -> Fut<'a> {
        done()
    }
    fn key_up<'a>(&'a self, _event: &'a KeyEvent) -> Fut<'a> {
        done()
    }
    fn dial_rotate<'a>(&'a self, _event: &'a DialRotateEvent) -> Fut<'a> {
        done()
    }
    fn dial_down<'a>(&'a self, _event: &'a DialPressEvent) -> Fut<'a> {
        done()
    }
    /// Mensaje de su ventana de ajustes
    fn to_plugin<'a>(&'a self, _event: &'a SendToPluginEvent) -> Fut<'a> {
        done()
    }
}

/// Todos los grupos de acciones de Prisma
static GROUPS: &[&dyn ActionGroup] = &[
    &crate::audio_actions::Audio,
    &crate::profiles::Wheel,
    &crate::profile_keys::Keys,
    &crate::widgets::Widgets,
    &crate::timers::Timers,
];

fn group_of(action: &str) -> Option<&'static dyn ActionGroup> {
    GROUPS.iter().copied().find(|g| g.owns(action))
}

/// Recibe los eventos de OpenDeck y los pasa al grupo de cada acción
pub struct Handler;

impl openaction::ActionEventHandler for Handler {
    async fn will_appear(&self, event: AppearEvent, outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if let Some(g) = group_of(&event.action) {
            g.appear(&event, outbound).await;
        }
        Ok(())
    }

    async fn will_disappear(&self, event: AppearEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if let Some(g) = group_of(&event.action) {
            g.disappear(&event).await;
        }
        Ok(())
    }

    async fn did_receive_settings(&self, event: DidReceiveSettingsEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if let Some(g) = group_of(&event.action) {
            g.settings(&event).await;
        }
        Ok(())
    }

    async fn key_down(&self, event: KeyEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if let Some(g) = group_of(&event.action) {
            g.key_down(&event).await;
        }
        Ok(())
    }

    async fn key_up(&self, event: KeyEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if let Some(g) = group_of(&event.action) {
            g.key_up(&event).await;
        }
        Ok(())
    }

    async fn dial_rotate(&self, event: DialRotateEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if let Some(g) = group_of(&event.action) {
            g.dial_rotate(&event).await;
        }
        Ok(())
    }

    async fn dial_down(&self, event: DialPressEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if let Some(g) = group_of(&event.action) {
            g.dial_down(&event).await;
        }
        Ok(())
    }

    async fn send_to_plugin(&self, event: SendToPluginEvent, _outbound: &mut OutboundEventManager) -> EventHandlerResult {
        if let Some(g) = group_of(&event.action) {
            g.to_plugin(&event).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cada acción del manifiesto tiene un grupo, y solo uno
    #[test]
    fn cada_accion_tiene_su_grupo() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!("../../manifest.json")).unwrap();
        let actions = manifest["Actions"].as_array().unwrap();
        assert!(!actions.is_empty());
        for a in actions {
            let uuid = a["UUID"].as_str().unwrap();
            let n = GROUPS.iter().filter(|g| g.owns(uuid)).count();
            assert_eq!(n, 1, "{uuid}: {n} grupos");
        }
    }
}
