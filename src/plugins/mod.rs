use bevy_app::{App, Plugin, Startup};
use bevy_ecs::system::Res;
use bevy_ecs::schedule::IntoScheduleConfigs;
use crate::plugins::server_plugin::host_functions::set_plugins_manager_bridge;
use plugins_manager::{rescan_plugins, PluginsManager};
use server_settings::{rescan_server_settings, setup_default_blocks, ServerSettings};

pub mod plugin_container;
pub mod plugins_manager;
pub mod resources_archive;
pub mod server_plugin;
pub mod server_settings;

#[derive(Default)]
pub struct PluginApp;

impl Plugin for PluginApp {
    fn build(&self, app: &mut App) {
        app.insert_resource(PluginsManager::default());
        app.insert_resource(ServerSettings::default());

        app.add_systems(Startup, register_plugins_manager_bridge);
        app.add_systems(Startup, setup_default_blocks);
        app.add_systems(Startup, rescan_plugins.after(setup_default_blocks));

        app.add_systems(Startup, rescan_server_settings.after(rescan_plugins));
    }
}

fn register_plugins_manager_bridge(plugins_manager: Res<PluginsManager>) {
    let _s = crate::span!("plugins.register_plugins_manager_bridge");
    set_plugins_manager_bridge(&plugins_manager);
}
