use ahash::AHashMap;
use bevy::prelude::Entity;
use common::utils::vec_remove_item;

#[derive(Default)]
pub struct InventoryWatchers {
    by_inventory: AHashMap<u64, Vec<Entity>>,
    by_entity: AHashMap<Entity, Vec<u64>>,
}

impl InventoryWatchers {
    pub fn insert_ticket(&mut self, inventory_id: u64, entity: Entity) -> bool {
        if self
            .by_inventory
            .get(&inventory_id)
            .is_some_and(|watchers| watchers.contains(&entity))
        {
            return false;
        }

        self.by_inventory.entry(inventory_id).or_default().push(entity);
        self.by_entity.entry(entity).or_default().push(inventory_id);
        true
    }

    pub fn remove_ticket(&mut self, inventory_id: &u64, entity: &Entity) {
        let remove_inventory = if let Some(vec) = self.by_inventory.get_mut(inventory_id) {
            vec_remove_item(vec, entity);
            vec.is_empty()
        } else {
            false
        };

        if remove_inventory {
            self.by_inventory.remove(inventory_id);
        }

        let remove_entity = if let Some(vec) = self.by_entity.get_mut(entity) {
            vec_remove_item(vec, inventory_id);
            vec.is_empty()
        } else {
            false
        };

        if remove_entity {
            self.by_entity.remove(entity);
        }
    }

    pub fn remove_all_entity_tickets(&mut self, entity: &Entity) {
        let Some(inventories) = self.by_entity.remove(entity) else {
            return;
        };

        for inventory_id in inventories {
            let remove_inventory = if let Some(vec) = self.by_inventory.get_mut(&inventory_id) {
                vec_remove_item(vec, entity);
                vec.is_empty()
            } else {
                false
            };

            if remove_inventory {
                self.by_inventory.remove(&inventory_id);
            }
        }
    }

    pub fn remove_all_inventory_tickets(&mut self, inventory_id: &u64) {
        let Some(entities) = self.by_inventory.remove(inventory_id) else {
            return;
        };

        for entity in entities {
            let remove_entity = if let Some(vec) = self.by_entity.get_mut(&entity) {
                vec_remove_item(vec, inventory_id);
                vec.is_empty()
            } else {
                false
            };

            if remove_entity {
                self.by_entity.remove(&entity);
            }
        }
    }

    pub fn get_inventory_watchers(&self, inventory_id: &u64) -> Option<&Vec<Entity>> {
        self.by_inventory.get(inventory_id)
    }
}
