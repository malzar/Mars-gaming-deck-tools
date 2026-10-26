use crate::SettingsValue;

use super::Coordinates;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct DidReceiveSettingsPayload {
	pub settings: SettingsValue,
	pub coordinates: Coordinates,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DidReceiveSettingsEvent {
	pub action: String,
	pub context: String,
	pub device: String,
	pub payload: DidReceiveSettingsPayload,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DidReceiveGlobalSettingsPayload {
	pub settings: SettingsValue,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DidReceiveGlobalSettingsEvent {
	pub payload: DidReceiveGlobalSettingsPayload,
}

/// Enviado por OpenDeck al pulsar "Ajustes del plugin" en la lista de plugins
/// (manifest con `HasSettingsInterface: true`). Añadido por MGDC.
#[derive(Clone, Debug, Deserialize)]
pub struct ShowSettingsInterfaceEvent {}
