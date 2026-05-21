use bevy::prelude::World;
use bevy_ecs::system::Command;
use common::chunks::block_position::BlockPositionTrait;

use super::worlds_manager::SharedWorldsManager;
use crate::{
    clients::client::Client,
    entities::{
        entity::{Position, Rotation},
        EntityComponent,
    },
    network::sync_players::PlayerSpawnEvent,
};

pub struct SpawnPlayer {
    world_slug: String,
    client: Client,
    position: Position,
    rotation: Rotation,
    components: Vec<EntityComponent>,
}

impl SpawnPlayer {
    pub fn create(
        world_slug: String,
        client: Client,
        position: Position,
        rotation: Rotation,
        components: Vec<EntityComponent>,
    ) -> Self {
        Self {
            world_slug,
            client,
            position,
            rotation,
            components,
        }
    }
}

impl Command for SpawnPlayer {
    fn apply(self, world: &mut World) {
        let (world_entity, is_chunk_loaded) = {
            let worlds_manager = world.resource::<SharedWorldsManager>();
            let worlds_manager = worlds_manager.write();
            let Some(mut world_manager) = worlds_manager.get_world_manager_mut(&self.world_slug) else {
                panic!("SpawnPlayer: world \"{}\" doesn't exists", self.world_slug);
            };

            let bundle = (self.position.clone(), self.rotation, self.client.clone());
            let world_entity = world_manager.spawn_player(self.position, bundle, self.components.clone());
            let is_chunk_loaded = world_manager
                .get_chunks_map()
                .is_chunk_loaded(&self.position.get_chunk_position());

            (world_entity, is_chunk_loaded)
        };

        self.client.set_world_entity(Some(world_entity.clone()));

        // Send world creation message
        self.client.network_send_spawn_pending();

        if is_chunk_loaded {
            world
                .write_message(PlayerSpawnEvent::new(world_entity.clone()))
                .unwrap();
        }
    }
}
