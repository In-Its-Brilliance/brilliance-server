use bevy_ecs::system::Res;
use common::{
    chunks::chunk_data::WorldMacroData,
    plugin_api::events::generage_world_macro::GenerateWorldMacroEvent,
    world_generator::traits::WorldGeneratorSettings,
    worlds_storage::taits::{IWorldStorage, WorldStorageData, WorldStorageSettings},
    WorldStorageManager,
};
use rand::Rng;

use crate::{
    launch_settings::LaunchSettings,
    network::runtime_plugin::RuntimePlugin,
    plugins::{plugins_manager::PluginsManager, server_settings::ServerSettings},
};

use super::worlds_manager::WorldsManager;

pub(crate) fn load_worlds(
    launch_settings: Res<LaunchSettings>,
    worlds_manager: Res<WorldsManager>,
    server_settings: Res<ServerSettings>,
    plugins_manager: Res<PluginsManager>,
) {
    if RuntimePlugin::is_stopped() {
        return;
    }

    let server_data_path = launch_settings.get_server_data_path();
    let storage_settings = WorldStorageSettings::from_path(server_data_path);

    let worlds_info = match WorldStorageManager::scan_worlds(storage_settings.clone()) {
        Ok(w) => w,
        Err(e) => {
            log::error!(target: "worlds", "&cWorlds scan error!");
            log::error!(target: "worlds", "&4Error: &c{}", e);
            RuntimePlugin::stop();
            return;
        }
    };

    for world_data in worlds_info.iter() {
        if !plugins_manager.has_world_generator(world_data.get_world_generator()) {
            log::error!(target: "worlds", "&cWorld generator \"{}\" not found to load \"{}\" world!", world_data.get_world_generator(), world_data.get_slug());
            RuntimePlugin::stop();
            return;
        }

        let world_generator_settings = WorldGeneratorSettings::from(world_data);

        let world_storage = match WorldStorageManager::init(storage_settings.clone(), world_data.get_slug().clone()) {
            Ok(s) => s,
            Err(e) => {
                log::error!(target: "worlds", "&cWorld storage init error!");
                log::error!(target: "worlds", "&4Error: &c{}", e);
                RuntimePlugin::stop();
                return;
            }
        };
        if let Err(e) = world_storage.validate_block_id_map(server_settings.get_block_id_map()) {
            log::error!(target: "worlds", "&cWorld validate_block_id_map error!");
            log::error!(target: "worlds", "&4Error: &c{}", e);
            RuntimePlugin::stop();
            return;
        }

        let create_result = worlds_manager.create_world(
            world_data.get_slug().clone(),
            world_storage,
            world_generator_settings.clone(),
        );
        if let Err(e) = create_result {
            log::error!(target: "worlds", "&cWorld create error!");
            log::error!(target: "worlds", "&4Error: &c{}", e);
            RuntimePlugin::stop();
            return;
        }
        log::info!(
            target: "worlds", "World &a\"{}\"&r loaded; &7generator: &8{} &7seed: &8{}",
            world_data.get_slug(), world_generator_settings.get_method(), world_generator_settings.get_seed(),
        );
    }

    let default_world = "default".to_string();
    let default_world_generator = "default".to_string();

    if worlds_manager.count() == 0 && !worlds_manager.has_world_with_slug(&default_world) {
        let result = create_new_world(
            default_world.clone(),
            None,
            default_world_generator,
            &*launch_settings,
            &*plugins_manager,
            &*worlds_manager,
        );
        match result {
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

fn create_new_world(
    slug: String,
    seed: Option<u64>,
    method: String,
    launch_settings: &LaunchSettings,
    plugins_manager: &PluginsManager,
    worlds_manager: &WorldsManager,
) -> Result<(), String> {
    let seed = match seed {
        Some(s) => s,
        None => rand::thread_rng().gen(),
    };

    let server_data_path = launch_settings.get_server_data_path();
    let storage_settings = WorldStorageSettings::from_path(server_data_path);
    let world_storage = match WorldStorageManager::init(storage_settings.clone(), slug.clone()) {
        Ok(s) => s,
        Err(e) => {
            return Err(format!("World storage init error: {}", e));
        }
    };

    if !plugins_manager.has_world_generator(&method) {
        RuntimePlugin::stop();
        return Err(format!(
            "&cWorld generator \"{}\" not found to create \"{}\" world!",
            method, slug
        ));
    }

    let plugin = plugins_manager
        .get_world_generator(&method)
        .expect("world_generator is required");

    let event = GenerateWorldMacroEvent::create(seed, method.clone(), None);
    let world_macro_data = match plugin.primary().has_event_handler::<GenerateWorldMacroEvent>() {
        true => match plugin.call_event_with_result(&event) {
            Ok(m) => m,
            Err(e) => {
                return Err(format!("Generation world macro data error: {}", e));
            }
        },
        false => WorldMacroData::default(),
    };
    let world_data = WorldStorageData::create(slug.clone(), seed, method.clone(), world_macro_data);
    world_storage.create_new(&world_data)?;

    let world_generator_settings = WorldGeneratorSettings::from(&world_data);
    worlds_manager.create_world(slug.clone(), world_storage, world_generator_settings)?;
    Ok(())
}
