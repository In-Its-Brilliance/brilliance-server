use bevy_app::{App, Plugin};

use crate::console::commands_executer::{CommandExecuter, CommandsHandler};

pub mod client;
pub mod console_commands;
pub mod clients_container;

use console_commands::{command_clear, command_parser_clear, command_parser_players, command_players};

pub struct ClientsPlugin;

impl Default for ClientsPlugin {
    fn default() -> Self {
        Self {}
    }
}

impl Plugin for ClientsPlugin {
    fn build(&self, app: &mut App) {
        let mut commands_handler = app.world_mut().get_resource_mut::<CommandsHandler>().unwrap();
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_players(), command_players));
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_clear(), command_clear));
    }
}
