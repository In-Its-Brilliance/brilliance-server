use bevy_app::{App, Plugin};

use crate::console::commands_executer::{CommandExecuter, CommandsHandler};

pub mod client;
pub mod clients_container;
pub mod console_commands;

use console_commands::{
    command_clear, command_give, command_kick, command_parser_clear, command_parser_give, command_parser_kick,
    command_parser_teleport, command_teleport,
};

pub struct ClientsPlugin;

impl Default for ClientsPlugin {
    fn default() -> Self {
        Self {}
    }
}

impl Plugin for ClientsPlugin {
    fn build(&self, app: &mut App) {
        let mut commands_handler = app.world_mut().get_resource_mut::<CommandsHandler>().unwrap();
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_give(), command_give));
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_kick(), command_kick));
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_clear(), command_clear));
        commands_handler.add_command_executer(CommandExecuter::new(command_parser_teleport(), command_teleport));
    }
}
