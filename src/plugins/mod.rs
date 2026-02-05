use bevy_app::{App, Plugin, Startup};
use bevy_ecs::schedule::IntoScheduleConfigs;
use plugins_manager::{rescan_plugins, PluginsManager};
use server_settings::{rescan_server_settings, setup_default_blocks, ServerSettings};

pub mod plugins_manager;
pub mod plugin_container;
pub mod resources_archive;
pub mod server_settings;
pub mod server_plugin;


#[derive(Default)]
pub struct PluginApp;

impl Plugin for PluginApp {
    fn build(&self, app: &mut App) {
        app.insert_resource(PluginsManager::default());
        app.insert_resource(ServerSettings::default());

        app.add_systems(Startup, setup_default_blocks);
        app.add_systems(Startup, rescan_plugins.after(setup_default_blocks));

        app.add_systems(Startup, rescan_server_settings.after(rescan_plugins));
    }
}
