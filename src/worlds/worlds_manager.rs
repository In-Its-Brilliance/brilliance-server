use std::{collections::BTreeMap, sync::Arc};

use ahash::HashMap;
use bevy::prelude::Resource;
use bevy::time::Time;
use bevy_ecs::system::Res;
use common::{
    chunks::chunk_data::BlockIndexType,
    world_generator::traits::WorldGeneratorSettings,
    worlds_storage::taits::{WorldInfo, WorldStorageSettings},
};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::{network::runtime_plugin::RuntimePlugin, plugins::plugins_manager::PluginsManager};

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

pub fn update_world_chunks(worlds_manager: Res<WorldsManager>, time: Res<Time>, plugins_manager: Res<PluginsManager>) {
    if RuntimePlugin::is_stopped() {
        return;
    }

    for (_key, world) in worlds_manager.get_worlds().iter() {

        let wasm_plugin_manager = plugins_manager
            .get_world_generator(world.read().get_world_generator())
            .expect("world_generator is required");

        world.write().update_chunks_state(time.delta(), wasm_plugin_manager);
    }
}
