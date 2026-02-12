use bevy_ecs::{message::MessageWriter, system::Res};

use crate::network::{
    client_network::{ClientNetwork, WorldEntity},
    runtime_plugin::RuntimePlugin,
    server::NetworkContainer,
    sync_players::PlayerSpawnEvent,
};

use super::worlds_manager::WorldsManager;

/// Iterates trough all worlds
/// and drain all their loaded chunks
pub fn on_chunk_loaded(
    worlds_manager: Res<WorldsManager>,
    network_container: Res<NetworkContainer>,
    mut player_spawn_events: MessageWriter<PlayerSpawnEvent>,
) {
    if RuntimePlugin::is_stopped() {
        return;
    }

    for world in worlds_manager.iter_worlds() {
        let loaded_chunks = world.get_chunks_map().drain_loaded_chunks().collect::<Vec<_>>();
        for chunk_position in loaded_chunks {
            let world_slug = world.get_slug().clone();
            let ecs = world.get_ecs();

            'entity_loop: for entity in ecs.get_chunk_entities(&chunk_position).unwrap() {
                let Some(network) = entity.get::<ClientNetwork>() else {
                    continue 'entity_loop;
                };

                let connected = network_container.is_connected(&network);
                if !connected {
                    continue 'entity_loop;
                }

                let world_entity = WorldEntity::new(world_slug.clone(), entity.id());
                player_spawn_events.write(PlayerSpawnEvent::new(world_entity));
            }
        }
    }
}
