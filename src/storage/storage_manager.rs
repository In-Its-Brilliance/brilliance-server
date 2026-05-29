use bevy_ecs::system::{Commands, Res};
use common::{
    server_storage::taits::IServerStorage, timed_lock, utils::srotage_settings::StorageSettings, ServerStorageManager,
};
use std::sync::Arc;

use crate::{
    launch_settings::LaunchSettings, plugins::server_plugin::host_functions::set_server_storage_bridge,
    runtime_plugin::RuntimePlugin, utils::Shared,
};

pub type SharedStorageManager = Shared<StorageManager>;

pub struct StorageManager {
    server_storage: Shared<ServerStorageManager>,
}

impl StorageManager {
    pub fn new(server_storage: Shared<ServerStorageManager>) -> Self {
        Self { server_storage }
    }

    pub fn read_server_storage(&self) -> parking_lot::RwLockReadGuard<'_, ServerStorageManager> {
        self.server_storage.read()
    }
}

pub(crate) fn init_server_storage(mut commands: Commands, launch_settings: Res<LaunchSettings>) {
    if RuntimePlugin::is_stopped() {
        return;
    }

    let storage_settings = StorageSettings::from_path(launch_settings.get_server_data_path());
    let storage = match ServerStorageManager::init(storage_settings) {
        Ok(storage) => storage,
        Err(e) => {
            log::error!(target: "storage", "&4Server storage init error:");
            log::error!(target: "storage", "&c{}", e);
            RuntimePlugin::stop();
            return;
        }
    };

    let server_storage = Shared::new(Arc::new(timed_lock!(storage, "server_storage_manager")));
    let storage_manager = SharedStorageManager::new(Arc::new(timed_lock!(
        StorageManager::new(server_storage),
        "storage_manager"
    )));
    set_server_storage_bridge(storage_manager.clone_inner());
    commands.insert_resource(storage_manager);
}
