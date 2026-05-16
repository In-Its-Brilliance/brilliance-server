use ahash::AHashMap;
use bevy::prelude::Entity;
use common::chunks::{
    block_position::ChunkBlockPosition,
    chunk_position::ChunkPosition,
    chunk_storage::{BlockInventory, ChunkStorage},
};

use super::inventory_load_state::InventoryWatchers;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryLocation {
    world_slug: String,
    chunk_position: ChunkPosition,
    section: u32,
    position: ChunkBlockPosition,
}

impl InventoryLocation {
    pub fn new(
        world_slug: impl Into<String>,
        chunk_position: ChunkPosition,
        section: u32,
        position: ChunkBlockPosition,
    ) -> Self {
        Self {
            world_slug: world_slug.into(),
            chunk_position,
            section,
            position,
        }
    }

    pub fn get_world_slug(&self) -> &String {
        &self.world_slug
    }

    pub fn get_chunk_position(&self) -> &ChunkPosition {
        &self.chunk_position
    }
}

#[derive(Default)]
pub struct InventoryState {
    inventory_watchers: InventoryWatchers,
    world_inventories: AHashMap<u64, InventoryLocation>,
}

impl InventoryState {
    pub fn register_world_inventory(
        &mut self,
        world_slug: impl Into<String>,
        chunk_position: ChunkPosition,
        block_inventory: &BlockInventory,
    ) {
        let world_slug = world_slug.into();
        let inventory_id = block_inventory.get_inventory().get_id();
        if self.world_inventories.contains_key(&inventory_id) {
            panic!(
                "duplicate world inventory id {} in world {} chunk {:?} section {} position {:?}",
                inventory_id,
                world_slug,
                chunk_position,
                block_inventory.get_section(),
                block_inventory.get_position()
            );
        }

        self.world_inventories.insert(
            inventory_id,
            InventoryLocation::new(
                world_slug,
                chunk_position,
                block_inventory.get_section(),
                *block_inventory.get_position(),
            ),
        );
    }

    pub fn register_chunk_inventories(
        &mut self,
        world_slug: impl Into<String>,
        chunk_position: ChunkPosition,
        chunk_storage: &ChunkStorage,
    ) {
        let world_slug = world_slug.into();
        for block_inventory in chunk_storage.get_inventories() {
            self.register_world_inventory(world_slug.clone(), chunk_position, block_inventory);
        }
    }

    pub fn unregister_chunk_inventories(&mut self, chunk_storage: &ChunkStorage) {
        for block_inventory in chunk_storage.get_inventories() {
            let inventory_id = block_inventory.get_inventory().get_id();
            self.inventory_watchers.remove_all_inventory_tickets(&inventory_id);
            self.world_inventories.remove(&inventory_id);
        }
    }

    pub fn get_inventory_location(&self, inventory_id: &u64) -> Option<&InventoryLocation> {
        self.world_inventories.get(inventory_id)
    }

    pub fn watch_inventory(&mut self, inventory_id: u64, entity: Entity) -> bool {
        self.inventory_watchers.insert_ticket(inventory_id, entity)
    }

    pub fn unwatch_inventory(&mut self, inventory_id: &u64, entity: &Entity) {
        self.inventory_watchers.remove_ticket(inventory_id, entity);
    }

    pub fn unwatch_entity(&mut self, entity: &Entity) {
        self.inventory_watchers.remove_all_entity_tickets(entity);
    }

    pub fn get_inventory_watchers(&self, inventory_id: &u64) -> Option<&Vec<Entity>> {
        self.inventory_watchers.get_inventory_watchers(inventory_id)
    }
}
