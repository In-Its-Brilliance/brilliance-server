use bevy::prelude::Res;
use bevy_ecs::message::MessageReader;
use common::{chunks::position::Vector3, plugin_api::events::player_spawn::PlayerSpawnEvent as PluginPlayerSpawnEvent};
use strum::IntoEnumIterator;

use crate::{
    clients::client::Client,
    entities::{
        entity::{Position, Rotation},
        entity_tag::EntityTagComponent,
        skin::EntitySkinComponent,
        EntityComponent,
    },
    network::{
        sync_entities::sync_entity_spawn,
        sync_players::{send_entities_for_player, PlayerSpawnEvent},
    },
    plugins::{plugins_manager::PluginsManager, server_settings::ServerSettings},
    worlds::worlds_manager::SharedWorldsManager,
};

/// Спавн игрока в мире
///
/// Вызывается при успешном подключении игрока если чанк прогружен
/// или при прогрузке чанка, если entity попал в загружающийся чанк.
pub(crate) fn on_player_spawn(
    worlds_manager: Res<SharedWorldsManager>,
    plugins_manager: Res<PluginsManager>,
    mut connection_events: MessageReader<PlayerSpawnEvent>,
    server_settings: Res<ServerSettings>,
) {
    #[cfg(feature = "trace")]
    let _span = bevy_utils::tracing::info_span!("on_player_spawn.on_player_spawn").entered();
    let _s = crate::span!("on_player_spawn.on_player_spawn");

    for event in connection_events.read() {
        let target_entity = event.world_entity.get_entity();
        let worlds_manager_guard = worlds_manager.write();
        let mut world_manager = worlds_manager_guard
            .get_world_manager_mut(event.world_entity.get_world_slug())
            .unwrap();

        send_entities_for_player(&world_manager, target_entity);

        let current_position = {
            let ecs = world_manager.get_ecs();
            let entity_ref = ecs.get_entity(target_entity).unwrap();
            *entity_ref.get::<Position>().unwrap()
        };

        let origin = current_position.to_network();
        let solid = world_manager.get_chunks_map().first_solid_block(
            origin,
            Vector3::new(0.0, -1.0, 0.0),
            512.0,
            &server_settings,
        );

        // Find first solid block under position
        let spawn_position = match solid {
            Some(block_pos) => {
                let p = block_pos.get_position();
                Position::new(p.x, p.y + 1.0, p.z)
            }
            None => current_position,
        };

        {
            let mut player_entity = world_manager.get_ecs_mut().entity_mut(target_entity);
            let mut position = player_entity.get_mut::<Position>().unwrap();
            *position = spawn_position;
        }

        let ecs = world_manager.get_ecs();
        let entity_ref = ecs.get_entity(target_entity).unwrap();

        let position = *entity_ref.get::<Position>().unwrap();
        let rotation = *entity_ref.get::<Rotation>().unwrap();
        let mut components: Vec<EntityComponent> = Vec::new();
        for comp in EntityComponent::iter() {
            match comp {
                EntityComponent::Tag(_) => {
                    if let Some(tag) = entity_ref.get::<EntityTagComponent>() {
                        components.push(EntityComponent::Tag(Some(tag.clone())));
                    }
                }
                EntityComponent::Skin(_) => {
                    if let Some(skin) = entity_ref.get::<EntitySkinComponent>() {
                        components.push(EntityComponent::Skin(Some(skin.clone())));
                    }
                }
            }
        }

        entity_ref
            .get::<Client>()
            .unwrap()
            .network_send_spawn(&position, &rotation, &components);

        if entity_ref.get::<EntitySkinComponent>().is_some() {
            sync_entity_spawn(&*world_manager, target_entity);
        }

        let plugin_event = PluginPlayerSpawnEvent::create(entity_ref.get::<Client>().unwrap().get_client_id());
        plugins_manager.dispatch_player_spawn_event(&plugin_event);
    }
}
