use bevy_ecs::system::{Res, ResMut};
use common::{
    world_generator::traits::WorldGeneratorSettings,
    worlds_storage::taits::{IWorldStorage, WorldInfo, WorldStorageSettings},
    WorldStorageManager,
};

use crate::{
    launch_settings::LaunchSettings,
    network::runtime_plugin::RuntimePlugin,
    plugins::{plugins_manager::PluginsManager, server_settings::ServerSettings},
};

use super::worlds_manager::WorldsManager;

pub(crate) fn load_worlds(
    launch_settings: Res<LaunchSettings>,
    mut worlds_manager: ResMut<WorldsManager>,
    server_settings: Res<ServerSettings>,
    plugins_manager: Res<PluginsManager>,
) {
    if RuntimePlugin::is_stopped() {
        return;
    }

    let server_data_path = launch_settings.get_server_data_path();
    let storage_settings = WorldStorageSettings::create(server_data_path);

    let worlds_info = match WorldStorageManager::scan_worlds(storage_settings.clone()) {
        Ok(w) => w,
        Err(e) => {
            log::error!(target: "worlds", "&cWorlds scan error!");
            log::error!(target: "worlds", "&4Error: &c{}", e);
            RuntimePlugin::stop();
            return;
        }
    };

    for world_info in worlds_info.iter() {
        if !plugins_manager.has_world_generator(world_info.get_world_generator()) {
            log::error!(target: "worlds", "&cWorld generator \"{}\" not found to load \"{}\" world!", world_info.get_world_generator(), world_info.get_slug());
            RuntimePlugin::stop();
            return;
        }

        let create_result = worlds_manager.create_world(
            world_info.clone(),
            storage_settings.clone(),
            WorldGeneratorSettings::create(
                Some(world_info.get_seed()),
                world_info.get_world_generator().clone(),
                None,
            ),
            server_settings.get_block_id_map(),
        );
        if let Err(e) = create_result {
            log::error!(target: "worlds", "&cWorlds load error!");
            log::error!(target: "worlds", "&4Error: &c{}", e);
            RuntimePlugin::stop();
            return;
        }
        log::info!(
            target: "worlds", "World &a\"{}\"&r loaded; &7generator: &8{} &7seed: &8{}",
            world_info.get_slug(), world_info.get_world_generator(), world_info.get_seed(),
        );
    }

    let default_world = "default".to_string();
    let default_world_generator = "default".to_string();

    if worlds_manager.count() == 0 && !worlds_manager.has_world_with_slug(&default_world) {
        let world_info = WorldInfo::create(default_world.clone(), None, default_world_generator.clone());
        let world_generator_settings =
            WorldGeneratorSettings::create(Some(world_info.get_seed()), world_info.get_world_generator(), None);

        if !plugins_manager.has_world_generator(&default_world_generator) {
            log::error!(target: "worlds", "&cWorld generator \"{}\" not found to create default world!", default_world_generator);
            RuntimePlugin::stop();
            return;
        }

        let world = worlds_manager.create_world(
            world_info,
            storage_settings,
            world_generator_settings,
            server_settings.get_block_id_map(),
        );
        match world {
            Ok(_) => {
                log::info!(target: "worlds", "&dDefault world &5\"{}\"&d was created", default_world);
            }
            Err(e) => {
                log::error!(target: "worlds", "&cError with creating &e\"{}\"&r world", default_world);
                log::error!(target: "worlds", "&cError: {}", e);
                RuntimePlugin::stop();
                return;
            }
        }
    }
}
