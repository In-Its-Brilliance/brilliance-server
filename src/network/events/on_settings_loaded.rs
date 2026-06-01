use crate::clients::client::Client;
use crate::network::server::NetworkEventListener;
use crate::worlds::commands::SpawnPlayer;
use crate::worlds::worlds_manager::SharedWorldsManager;
use bevy::prelude::{Commands, Res};
use bevy_ecs::message::Message;
use common::utils::events::EventReader;

#[derive(Message)]
pub struct PlayerSettingsLoadedEvent {
    client: Client,
}

impl PlayerSettingsLoadedEvent {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

pub fn on_settings_loaded(
    mut commands: Commands,
    events: Res<NetworkEventListener<PlayerSettingsLoadedEvent>>,
    worlds_manager: Res<SharedWorldsManager>,
) {
    let _s = crate::span!("events.on_settings_loaded");
    for event in events.0.iter_events() {
        let default_world = "default".to_string();
        if !worlds_manager.read().has_world_with_slug(&default_world) {
            panic!("default world is not found");
        };

        commands.queue(SpawnPlayer::create(default_world, event.client.clone()));

        // let skin = EntitySkinComponent::create(NetworkEntitySkin::Generic);
        // commands.queue(_UpdatePlayerComponent::_create(
        //     event.client.clone(),
        //     EntityComponent::Skin(Some(skin)),
        // ));
    }
}
