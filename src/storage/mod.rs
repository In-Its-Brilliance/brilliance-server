pub mod storage_manager;

use bevy_app::{App, Plugin, Startup};
use bevy_ecs::schedule::IntoScheduleConfigs;
use storage_manager::init_server_storage;

use crate::plugins::plugins_manager::rescan_plugins;

#[derive(Default)]
pub struct StoragePlugin;

impl Plugin for StoragePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, init_server_storage.before(rescan_plugins));
    }
}
