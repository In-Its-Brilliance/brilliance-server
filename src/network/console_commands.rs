use bevy_ecs::world::World;
use common::commands::command::{Arg, Command, CommandMatch};

use crate::{clients::clients_container::SharedClientsContainer, console::console_sender::ConsoleSenderType};

pub(crate) fn command_parser_kick() -> Command {
    Command::new("kick".to_owned())
        .arg(Arg::new("login".to_owned()).required(true).completer(crate::clients::console_commands::complete_players))
        .arg(Arg::new("message".to_owned()).required(false))
}

pub(crate) fn command_kick(
    world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    args: CommandMatch,
) -> Result<(), String> {
    let login = args.get_arg::<String, _>("login")?.clone();

    let Some(clients) = world.get_resource::<SharedClientsContainer>() else {
        sender.send_console_message("&cClients container is not loaded".to_string());
        return Ok(());
    };
    let clients_guard = clients.read();

    let Some(client) = clients_guard.get_by_login(&login) else {
        sender.send_console_message(format!("&cPlayer with login \"{}\" not found", login));
        return Ok(());
    };

    let message = match args.get_arg::<String, _>("message") {
        Ok(m) => m,
        Err(_) => "-".to_string(),
    };
    client.disconnect(Some(message.clone()));
    sender.send_console_message(format!(
        "Admin &a{}&r kicked player &a{}&r with reason: &e{}",
        sender.get_name(),
        login,
        message
    ));
    return Ok(());
}
