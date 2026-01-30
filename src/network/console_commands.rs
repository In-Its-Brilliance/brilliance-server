use bevy_ecs::world::World;
use common::commands::command::{Arg, Command, CommandMatch};

use crate::{console::console_sender::ConsoleSenderType, network::clients_container::ClientsContainer};

pub(crate) fn command_parser_kick() -> Command {
    Command::new("kick".to_owned())
        .arg(Arg::new("login".to_owned()).required(true))
        .arg(Arg::new("message".to_owned()).required(false))
}

pub(crate) fn command_kick(
    world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    args: CommandMatch,
) -> Result<(), String> {
    let login = args.get_arg::<String, _>("login")?.clone();

    let clients = world.resource::<ClientsContainer>();

    let Some(client) = clients.get_by_login(&login) else {
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
