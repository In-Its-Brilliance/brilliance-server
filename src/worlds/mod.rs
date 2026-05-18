use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;
pub mod commands;
pub mod load_worlds;

use crate::{
    console::commands_executer::{CommandExecuter, CommandsHandler},
    plugins::server_settings::rescan_server_settings,
};
use common::timed_lock;

use self::{
    console_commands::{command_parser_teleport, command_parser_world, command_teleport, command_world},
    worlds_manager::{update_world_chunks, SharedWorldsManager, WorldsManager},
};
use crate::plugins::server_plugin::host_functions::set_worlds_manager_bridge;
use std::sync::Arc;

pub mod chunks;
pub mod console_commands;
pub mod ecs;
pub mod on_chunk_loaded;
pub mod world_manager;
pub mod worlds_manager;

#[derive(Default)]
pub struct WorldsHandlerPlugin;

impl Plugin for WorldsHandlerPlugin {
    fn build(&self, app: &mut App) {
        let mut commands_handler = app.world_mut().get_resource_mut::<CommandsHandler>().unwrap();
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_world(), command_world));
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_teleport(), command_teleport));

        let worlds_manager =
            SharedWorldsManager::new(Arc::new(timed_lock!(WorldsManager::default(), "worlds_manager")));
        app.insert_resource(worlds_manager);
        app.add_systems(Startup, register_worlds_manager_bridge);

        app.add_systems(Startup, load_worlds::load_worlds.after(rescan_server_settings));
        app.add_systems(Update, update_world_chunks);
        app.add_systems(Update, on_chunk_loaded::on_chunk_loaded);
    }
}

pub fn register_worlds_manager_bridge(worlds_manager: bevy_ecs::system::Res<SharedWorldsManager>) {
    let _s = crate::span!("worlds.register_worlds_manager_bridge");
    set_worlds_manager_bridge(worlds_manager.clone_inner());
}
