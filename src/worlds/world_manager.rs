use super::ecs::Ecs;
use crate::entities::entity::{Position, Rotation};
use crate::entities::EntityComponent;
use crate::network::client_network::WorldEntity;
use crate::plugins::server_plugin::plugin_instance::WASMPluginManager;
use crate::worlds::chunks::chunks_map::ChunkMap;
use crate::CHUNKS_DISTANCE;
use bevy_ecs::bundle::Bundle;
use common::chunks::block_position::BlockPositionTrait;
use common::chunks::chunk_position::ChunkPosition;
use common::world_generator::traits::WorldGeneratorSettings;
use common::WorldStorageManager;
use network::messages::ServerMessages;
use std::sync::Arc;
use std::time::Duration;

pub struct ChunkChanged {
    pub old_chunk: ChunkPosition,
    pub new_chunk: ChunkPosition,
    pub abandoned_chunks: Vec<ChunkPosition>,
    pub new_chunks: Vec<ChunkPosition>,
}

pub struct WorldManager {
    slug: String,
    ecs: Ecs,
    chunks_map: ChunkMap,
}

impl WorldManager {
    pub fn new(
        slug: String,
        world_storage: WorldStorageManager,
        world_generator_settings: WorldGeneratorSettings,
    ) -> Result<Self, String> {
        Ok(WorldManager {
            slug,
            ecs: Ecs::new(),
            chunks_map: ChunkMap::new(world_storage, world_generator_settings),
        })
    }

    pub fn get_world_generator(&self) -> String {
        let world_generator_settings = self.chunks_map.get_world_generator_settings();
        world_generator_settings.get_method().clone()
    }

    pub fn get_ecs(&self) -> &Ecs {
        &self.ecs
    }

    pub fn get_ecs_mut(&mut self) -> &mut Ecs {
        &mut self.ecs
    }

    pub fn get_chunks_map(&self) -> &ChunkMap {
        &self.chunks_map
    }

    pub fn get_chunks_map_mut(&mut self) -> &mut ChunkMap {
        &mut self.chunks_map
    }

    pub fn get_slug(&self) -> &String {
        &self.slug
    }

    pub fn get_chunks_count(&self) -> usize {
        self.get_chunks_map().count()
    }

    pub fn spawn_player<B: Bundle>(
        &mut self,
        position: Position,
        bundle: B,
        components: Vec<EntityComponent>,
    ) -> WorldEntity {
        let entity = self.get_ecs_mut().spawn(bundle, position.get_chunk_position());

        let mut entity_ecs = self.get_ecs_mut().entity_mut(entity);
        if components.len() > 0 {
            for component in components {
                match component {
                    EntityComponent::Tag(c) => {
                        if let Some(c) = c {
                            entity_ecs.insert(c);
                        }
                    }
                    EntityComponent::Skin(c) => {
                        if let Some(c) = c {
                            entity_ecs.insert(c);
                        }
                    }
                }
            }
        }

        self.get_chunks_map_mut()
            .start_chunks_render(entity, &position.get_chunk_position(), CHUNKS_DISTANCE);

        WorldEntity::new(self.get_slug().clone(), entity)
    }

    /// Records the player's movement and updates his position in ECS.
    ///
    /// Returns boolean if player changed his chunk and his despawned chunks
    pub fn player_move(
        &mut self,
        world_entity: &WorldEntity,
        position: Position,
        rotation: Rotation,
    ) -> Option<ChunkChanged> {
        let mut changed_chunks: Option<ChunkChanged> = None;

        let mut player_entity = self.ecs.entity_mut(world_entity.get_entity());
        let mut old_position = player_entity.get_mut::<Position>().unwrap();

        let old_chunk = old_position.get_chunk_position();
        let new_chunk = position.get_chunk_position();
        let chunk_changed = old_chunk != new_chunk;
        if chunk_changed {
            let chunks = self.chunks_map.update_chunks_render(
                world_entity.get_entity(),
                &old_chunk,
                &new_chunk,
                CHUNKS_DISTANCE,
            );
            changed_chunks = Some(chunks);
        }
        *old_position = position;
        let mut old_rotation = player_entity.get_mut::<Rotation>().unwrap();
        *old_rotation = rotation;

        if chunk_changed {
            self.ecs
                .entity_moved_chunk(&world_entity.get_entity(), &old_chunk, &new_chunk);
        }
        changed_chunks
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.chunks_map.save()?;
        log::info!(target: "worlds", "World &a\"{}\"&r saved", self.get_slug());
        Ok(())
    }

    pub fn despawn_player(&mut self, world_entity: &WorldEntity) {
        self.get_chunks_map_mut().stop_chunks_render(world_entity.get_entity());

        let player_entity = self.ecs.get_entity(world_entity.get_entity()).unwrap();
        let chunk_position = match player_entity.get::<Position>() {
            Some(p) => Some(p.get_chunk_position()),
            None => None,
        };

        self.get_ecs_mut().despawn(world_entity.get_entity(), chunk_position);
    }

    /// Proxy for sending update_chunks
    pub fn update_chunks_state(&mut self, delta: Duration, wasm_plugin_manager: Arc<WASMPluginManager>) {
        let world_slug = self.get_slug().clone();
        self.chunks_map
            .update_chunks_state(delta, &world_slug, wasm_plugin_manager);
    }

    pub fn get_network_chunk_bytes(&self, chunk_position: &ChunkPosition) -> Option<ServerMessages> {
        match self.get_chunks_map().get_chunk_column(&chunk_position) {
            Some(chunk_column) => {
                if !chunk_column.is_loaded() {
                    return None;
                }
                Some(chunk_column.build_network_format())
            }
            None => None,
        }
    }
}
