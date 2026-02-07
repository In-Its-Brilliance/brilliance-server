use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use ahash::HashMap;
use bevy::prelude::Resource;
use bevy::time::Time;
use bevy_ecs::system::Res;
use common::{
    chunks::chunk_data::BlockIndexType,
    world_generator::traits::WorldGeneratorSettings,
    worlds_storage::taits::{IWorldStorage, WorldInfo, WorldStorageSettings},
    WorldStorageManager,
};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::world_manager::WorldManager;

type WorldsType = HashMap<String, Arc<RwLock<WorldManager>>>;

/// Contains and managers of all worlds of the server
#[derive(Resource)]
pub struct WorldsManager {
    worlds: WorldsType,
}

impl Default for WorldsManager {
    fn default() -> Self {
        WorldsManager {
            worlds: Default::default(),
        }
    }
}

impl WorldsManager {
    pub fn scan_worlds(
        &mut self,
        storage_settings: WorldStorageSettings,
        block_id_map: &BTreeMap<BlockIndexType, String>,
    ) -> Result<(), String> {
        let mut worlds_info = match WorldStorageManager::scan_worlds(storage_settings.clone()) {
            Ok(w) => w,
            Err(e) => {
                return Err(e.to_string());
            }
        };
        for world_info in worlds_info.drain(..) {
            if let Err(e) = self.create_world(
                world_info.clone(),
                storage_settings.clone(),
                WorldGeneratorSettings::create(
                    Some(world_info.get_seed()),
                    world_info.get_world_generator().clone(),
                    None,
                ),
                block_id_map,
            ) {
                return Err(e.to_string());
            };
            log::info!(
                target: "worlds", "World &a\"{}\"&r loaded; &7generator: &8{} &7seed: &8{}",
                world_info.get_slug(), world_info.get_world_generator(), world_info.get_seed(),
            );
        }
        Ok(())
    }

    pub fn has_world_with_slug(&self, slug: &String) -> bool {
        self.worlds.contains_key(slug)
    }

    pub fn save_all(&self) -> Result<(), String> {
        for (_world_slug, world) in self.worlds.iter() {
            world.write().save()?;
        }
        Ok(())
    }

    pub fn create_world(
        &mut self,
        world_info: WorldInfo,
        storage_settings: WorldStorageSettings,
        world_generator_settings: WorldGeneratorSettings,
        block_id_map: &BTreeMap<BlockIndexType, String>,
    ) -> Result<(), String> {
        if self.worlds.contains_key(world_info.get_slug()) {
            return Err(format!(
                "&cWorld with slug &4\"{}\"&c already exists",
                world_info.get_slug()
            ));
        }
        let world = match WorldManager::new(
            world_info.clone(),
            storage_settings,
            world_generator_settings,
            block_id_map,
        ) {
            Ok(w) => w,
            Err(e) => return Err(format!("&cWorld &4\"{}\"&c error: {}", world_info.get_slug(), e)),
        };
        self.worlds
            .insert(world_info.get_slug().clone(), Arc::new(RwLock::new(world)));
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.worlds.len()
    }

    pub fn get_worlds(&self) -> &WorldsType {
        &self.worlds
    }

    pub fn get_world_manager(&self, key: &String) -> Option<RwLockReadGuard<'_, WorldManager>> {
        match self.worlds.get(key) {
            Some(w) => Some(w.read()),
            None => None,
        }
    }

    pub fn get_world_manager_mut(&self, key: &String) -> Option<RwLockWriteGuard<'_, WorldManager>> {
        match self.worlds.get(key) {
            Some(w) => Some(w.write()),
            None => return None,
        }
    }
}

pub fn update_world_chunks(worlds_manager: Res<WorldsManager>, time: Res<Time>) {
    for (_key, world) in worlds_manager.get_worlds().iter() {
        world.write().update_chunks(time.delta());
    }
}
