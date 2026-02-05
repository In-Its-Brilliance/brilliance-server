use bevy_app::{App, Plugin, Startup};
use bevy_ecs::schedule::IntoScheduleConfigs;
use plugin_manager::{rescan_plugins, PluginManager};
use server_settings::{rescan_server_settings, setup_default_blocks, ServerSettings};

pub mod plugin_manager;
pub mod plugin_instance;
pub mod resources_archive;
pub mod server_settings;


#[derive(Default)]
pub struct PluginApp;

impl Plugin for PluginApp {
    fn build(&self, app: &mut App) {
        app.insert_resource(PluginManager::default());
        app.insert_resource(ServerSettings::default());

        app.add_systems(Startup, setup_default_blocks);
        app.add_systems(Startup, rescan_plugins.after(setup_default_blocks));

        app.add_systems(Startup, rescan_server_settings.after(rescan_plugins));
    }
}
