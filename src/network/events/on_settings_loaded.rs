use crate::network::server::NetworkEventListener;
use crate::worlds::commands::SpawnPlayer;
use crate::worlds::worlds_manager::SharedWorldsManager;
use crate::{
    clients::client::Client,
    entities::{
        entity::{Position, Rotation},
        entity_tag::EntityTagComponent,
        skin::EntitySkinComponent,
        EntityComponent,
    },
};
use bevy::prelude::{Commands, Res};
use bevy_ecs::message::Message;
use common::utils::events::EventReader;
use network::entities::{entity_tag::EntityTagData, EntitySkinData};

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

        let mut components: Vec<EntityComponent> = Default::default();

        // let skin = EntitySkinComponent::create(EntitySkinData::Fixed("test://godot_robot.glb".into()));
        let skin = EntitySkinComponent::create(EntitySkinData::Generic);
        components.push(EntityComponent::Skin(Some(skin)));

        let client_info = event.client.get_client_info().unwrap();
        let tag = EntityTagComponent::create(EntityTagData::create(client_info.get_login().clone(), None, None, None));
        components.push(EntityComponent::Tag(Some(tag)));

        commands.queue(SpawnPlayer::create(
            default_world,
            event.client.clone(),
            Position::new(0.0, 100.0, 0.0),
            Rotation::new(0.0, 0.0),
            components,
        ));

        // let skin = EntitySkinComponent::create(NetworkEntitySkin::Generic);
        // commands.queue(UpdatePlayerComponent::create(
        //     event.client.clone(),
        //     EntityComponent::Skin(Some(skin)),
        // ));
    }
}
