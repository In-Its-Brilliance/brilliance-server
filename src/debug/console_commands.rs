use bevy_ecs::world::World;
use common::commands::command::{Command, CommandMatch};
use common::utils::colors::Color;

use crate::{console::console_sender::ConsoleSenderType, debug::tps_counter::TpsCounter};

pub(crate) fn command_parser_tps() -> Command {
    Command::new("tps".to_owned())
}

pub(crate) fn command_tps(
    world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    _args: CommandMatch,
) -> Result<(), String> {
    let tps_counter = world.resource::<TpsCounter>();
    sender.send_console_message(format!("Server TPS: {}", tps_counter.get_tps()));
    return Ok(());
}

pub(crate) fn command_parser_color() -> Command {
    Command::new("colors".to_owned())
}

pub(crate) fn command_color(
    _world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    _args: CommandMatch,
) -> Result<(), String> {
    for color in Color::iter() {
        sender.send_console_message(format!("{}\\{} {}", color.to_code(), color.to_code(), color.to_str()));
    }
    Ok(())
}
