//! Perfiles: cambiar de perfil con la rueda del MSD-PRO, por niveles, y
//! crear, borrar y ordenar perfiles.
//!
//! OpenDeck solo deja cambiar de perfil a su propia acción "Switch Profile".
//! Para usarla desde la rueda, Prisma le dice a OpenDeck que el deck tiene unas
//! teclas más (10 en el MSD-PRO, 12 en el MSD-ONE) que solo existen en el
//! editor: OpenDeck las pinta en una fila debajo de las ruedas ("touchpoints").
//! Ahí va un "Switch Profile" por perfil. La rueda recorre esa lista y, al
//! elegir, Prisma "pulsa" esa tecla virtual: OpenDeck cambia de perfil él mismo.
//!
//! - `store`: los ficheros de perfiles de OpenDeck (leer, guardar, copias)
//! - `slots`: las teclas virtuales (sus imágenes, cuántas hay, pulsarlas)
//! - `arrange`: ordenar la fila virtual de cada perfil al arrancar
//! - `manage`: crear, renombrar y borrar perfiles
//! - `nav`: la navegación por niveles (lo elegido y el nivel de cada deck)
//! - `wheel`: la rueda de perfiles (girar, pulsar, tocar la pantalla)
//! - `draw`: el dibujo de la rueda, con el icono del perfil

mod arrange;
mod draw;
mod manage;
mod nav;
mod slots;
mod store;
mod wheel;

use openaction::{AppearEvent, DialPressEvent, DialRotateEvent, DidReceiveSettingsEvent, OutboundEventManager};

use crate::actions::{ActionGroup, Fut, done};

pub use arrange::prepare;
pub use draw::{anim_task, compose, redraw_device};
#[cfg(test)]
pub use draw::{SlotView, render_slot};
pub use manage::{check_delete, check_name, check_rename, create_profile, delete_profile, rename_profile};
pub use nav::{go, rename as rename_in_wheel, up, view};
pub use slots::{forget_images, intercept, switch_device, virtual_count};
pub use store::{active_profile, list_profiles, setting_in_profiles};
pub use wheel::swipe;

/// La rueda de perfiles
pub const UUID: &str = "com.malzar.prisma.profiles";
/// Filas de teclas virtuales por deck (MSD-PRO: 2 × 5 = 10; MSD-ONE: 2 × 6 = 12)
pub const VIRTUAL_ROWS: u8 = 2;

/// El grupo de la rueda de perfiles
pub struct Wheel;

impl ActionGroup for Wheel {
    fn owns(&self, action: &str) -> bool {
        action == UUID
    }

    fn appear<'a>(&'a self, e: &'a AppearEvent, outbound: &'a mut OutboundEventManager) -> Fut<'a> {
        Box::pin(wheel::appear(&e.context, &e.device, e.payload.coordinates.column, &e.payload.settings, outbound))
    }

    fn disappear<'a>(&'a self, e: &'a AppearEvent) -> Fut<'a> {
        Box::pin(wheel::disappear(&e.context))
    }

    fn settings<'a>(&'a self, e: &'a DidReceiveSettingsEvent) -> Fut<'a> {
        let (context, settings) = (e.context.clone(), e.payload.settings.clone());
        tokio::spawn(async move { wheel::settings_changed(&context, &settings).await });
        done()
    }

    fn dial_rotate<'a>(&'a self, e: &'a DialRotateEvent) -> Fut<'a> {
        let (context, ticks) = (e.context.clone(), e.payload.ticks);
        tokio::spawn(async move { wheel::rotate(&context, ticks).await });
        done()
    }

    fn dial_down<'a>(&'a self, e: &'a DialPressEvent) -> Fut<'a> {
        let context = e.context.clone();
        tokio::spawn(async move { wheel::press(&context).await });
        done()
    }
}
