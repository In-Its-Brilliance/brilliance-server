use bevy_ecs::system::{Res, ResMut};
use common::{
    world_generator::traits::WorldGeneratorSettings,
    worlds_storage::taits::{WorldInfo, WorldStorageSettings},
};

use crate::{
    launch_settings::LaunchSettings, network::runtime_plugin::RuntimePlugin, plugins::server_settings::ServerSettings,
};

use super::worlds_manager::WorldsManager;

pub(crate) fn load_worlds(
    launch_settings: Res<LaunchSettings>,
    mut worlds_manager: ResMut<WorldsManager>,
    server_settings: Res<ServerSettings>,
) {
    if RuntimePlugin::is_stopped() {
        return;
    }

    let server_data_path = launch_settings.get_server_data_path();
    let world_storage_settings = WorldStorageSettings::create(server_data_path);

    if let Err(e) = worlds_manager.scan_worlds(world_storage_settings.clone(), server_settings.get_block_id_map()) {
        log::error!(target: "worlds", "&cWorlds loading error!");
        log::error!(target: "worlds", "&4Error: &c{}", e);
        RuntimePlugin::stop();
        return;
    }

    let default_world = "default".to_string();
    if worlds_manager.count() == 0 && !worlds_manager.has_world_with_slug(&default_world) {
        let world_info = WorldInfo::create(default_world.clone(), None, "default");
        let world_generator_settings = WorldGeneratorSettings::create(Some(world_info.get_seed()), "default", None);

        let world = worlds_manager.create_world(
            world_info,
            world_storage_settings,
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
