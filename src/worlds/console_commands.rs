use crate::console::console_sender::ConsoleSenderType;
use crate::entities::entity::{Position, Rotation};
use crate::network::client_network::ClientNetwork;
use crate::network::events::on_player_move::move_player;
use bevy_ecs::world::World;
use common::commands::command::{Arg, Command, CommandMatch};

use super::worlds_manager::WorldsManager;

pub(crate) fn command_parser_world() -> Command {
    Command::new("world".to_string())
        .subcommand_required(true)
        .subcommand(Command::new("list".to_owned()))
}

pub(crate) fn command_world(
    world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    args: CommandMatch,
) -> Result<(), String> {
    if let Some(world_subcommand) = args.subcommand() {
        match world_subcommand.get_name().as_str() {
            "list" => {
                let worlds_manager = world.resource_mut::<WorldsManager>();
                if worlds_manager.count() == 0 {
                    sender.send_console_message("Worlds list is empty".to_string());
                    return Ok(());
                }
                sender.send_console_message("Worlds list:".to_string());
                for world in worlds_manager.iter_worlds() {
                    sender.send_console_message(format!(
                        " - {} (loaded chunks: {})",
                        world.get_slug(),
                        world.get_chunks_count()
                    ));
                }
            }
            _ => {
                sender.send_console_message("Error".to_string());
            }
        }
    }
    return Ok(());
}

pub(crate) fn command_parser_teleport() -> Command {
    Command::new("tp".to_owned())
        .arg(Arg::new("x".to_owned()).required(true))
        .arg(Arg::new("y".to_owned()).required(true))
        .arg(Arg::new("z".to_owned()).required(true))
}

pub(crate) fn command_teleport(
    world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    args: CommandMatch,
) -> Result<(), String> {
    let x = args.get_arg::<f32, _>("x")?.clone();
    let y = args.get_arg::<f32, _>("y")?.clone();
    let z = args.get_arg::<f32, _>("z")?.clone();

    let worlds_manager = world.resource::<WorldsManager>();

    let client = match sender.as_any().downcast_ref::<ClientNetwork>() {
        Some(c) => c,
        None => {
            sender.send_console_message("This command is allowed to be used only for players".to_string());
            return Ok(());
        }
    };

    let position = Position::new(x, y, z);
    let rotation = Rotation::new(0.0, 0.0);

    let Some(world_entity) = client.get_world_entity() else {
        sender.send_console_message(format!(
            "Player \"{}\" is not in the world",
            client.get_client_info().unwrap().get_login()
        ));
        return Ok(());
    };

    let mut world_manager = worlds_manager
        .get_world_manager_mut(&world_entity.get_world_slug())
        .unwrap();

    move_player(&mut *world_manager, &world_entity, position, rotation);
    return Ok(());
}
