use bevy::prelude::Resource;
use bevy::time::Time;
use bevy_ecs::system::Res;
use common::{world_generator::traits::WorldGeneratorSettings, WorldStorageManager};
use dashmap::DashMap;

use crate::inventory::SharedInventoryManager;
use crate::{plugins::plugins_manager::PluginsManager, runtime_plugin::RuntimePlugin, utils::Shared};

use super::world_manager::WorldManager;

type WorldsType = DashMap<String, WorldManager>;

/// Contains and managers of all worlds of the server
#[derive(Resource)]
pub struct WorldsManager {
    worlds: WorldsType,
}

pub type SharedWorldsManager = Shared<WorldsManager>;

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
        for entry in self.worlds.iter() {
            entry.value().save()?;
        }
        Ok(())
    }

    pub fn create_world(
        &self,
        slug: String,
        world_storage: WorldStorageManager,
        world_generator_settings: WorldGeneratorSettings,
    ) -> Result<(), String> {
        if self.worlds.contains_key(&slug) {
            return Err(format!("&cWorld with slug &4\"{}\"&c already exists", slug));
        }

        let world = match WorldManager::new(slug.clone(), world_storage, world_generator_settings) {
            Ok(w) => w,
            Err(e) => return Err(format!("&cWorld &4\"{}\"&c error: {}", slug, e)),
        };
        self.worlds.insert(slug, world);
        Ok(())
    }

    pub fn count(&self) -> usize {
        self.worlds.len()
    }

    pub fn iter_worlds(&self) -> impl Iterator<Item = WorldRefMulti<'_>> {
        self.worlds.iter().map(|g| WorldRefMulti { _guard: g })
    }

    pub fn iter_worlds_mut(&self) -> impl Iterator<Item = WorldRefMultiMut<'_>> {
        self.worlds.iter_mut().map(|g| WorldRefMultiMut { _guard: g })
    }

    pub fn get_world_manager(&self, key: &String) -> Option<WorldRef<'_>> {
        self.worlds.get(key).map(|g| WorldRef { _guard: g })
    }

    pub fn get_world_manager_mut(&self, key: &String) -> Option<WorldRefMut<'_>> {
        self.worlds.get_mut(key).map(|g| WorldRefMut { _guard: g })
    }
}

pub struct WorldRef<'a> {
    _guard: dashmap::mapref::one::Ref<'a, String, WorldManager>,
}

impl<'a> std::ops::Deref for WorldRef<'a> {
    type Target = WorldManager;
    fn deref(&self) -> &WorldManager {
        self._guard.value()
    }
}

pub struct WorldRefMut<'a> {
    _guard: dashmap::mapref::one::RefMut<'a, String, WorldManager>,
}

impl<'a> std::ops::Deref for WorldRefMut<'a> {
    type Target = WorldManager;
    fn deref(&self) -> &WorldManager {
        self._guard.value()
    }
}

impl<'a> std::ops::DerefMut for WorldRefMut<'a> {
    fn deref_mut(&mut self) -> &mut WorldManager {
        self._guard.value_mut()
    }
}

// For iter

pub struct WorldRefMulti<'a> {
    _guard: dashmap::mapref::multiple::RefMulti<'a, String, WorldManager>,
}

impl<'a> std::ops::Deref for WorldRefMulti<'a> {
    type Target = WorldManager;
    fn deref(&self) -> &WorldManager {
        self._guard.value()
    }
}

pub struct WorldRefMultiMut<'a> {
    _guard: dashmap::mapref::multiple::RefMutMulti<'a, String, WorldManager>,
}

impl<'a> std::ops::Deref for WorldRefMultiMut<'a> {
    type Target = WorldManager;
    fn deref(&self) -> &WorldManager {
        self._guard.value()
    }
}

impl<'a> std::ops::DerefMut for WorldRefMultiMut<'a> {
    fn deref_mut(&mut self) -> &mut WorldManager {
        self._guard.value_mut()
    }
}

pub fn update_world_chunks(
    worlds_manager: Res<SharedWorldsManager>,
    time: Res<Time>,
    plugins_manager: Res<PluginsManager>,
    inventory_manager: Res<SharedInventoryManager>,
) {
    let mut inventory_manager = inventory_manager.write();
    let _s = crate::span!("worlds.update_world_chunks");
    if RuntimePlugin::is_stopped() {
        return;
    }
    let worlds_manager_guard = worlds_manager.read();
    for mut world in worlds_manager_guard.iter_worlds_mut() {
        let loaded_chunks = world.get_chunks_map().drain_loaded_chunks().collect::<Vec<_>>();
        for chunk_position in loaded_chunks {
            let world_slug = world.get_slug().clone();
            let Some(chunk_column_arc) = world.get_chunks_map().get_chunk_column_arc(&chunk_position) else {
                continue;
            };
            let chunk_storage = chunk_column_arc.read().get_chunk_storage().clone();
            inventory_manager.state_mut().register_chunk_inventories(
                world_slug.clone(),
                chunk_position,
                &chunk_storage,
            );
        }

        let wasm_plugin_manager = plugins_manager
            .get_world_generator(&world.get_world_generator())
            .expect("world_generator is required");

        world.update_chunks_state(time.delta(), wasm_plugin_manager, &mut inventory_manager);
    }
}
