use bevy_ecs::world::World;
use common::commands::command::{Command, CommandMatch};

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
    sender.send_console_message(format!("TPS: {}", tps_counter.get_tps()));
    return Ok(());
}
