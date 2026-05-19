use bevy_ecs::world::World;
use common::commands::command::{Arg, Command, CommandMatch};
use common::inventory::item::Item;

use crate::{
    clients::clients_container::SharedClientsContainer,
    console::console_sender::ConsoleSenderType,
    items_manager::items_manager::SharedItemsManager,
    network::sync_inventory::send_inventory_changes_to_client,
};

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

    let clients = world.resource::<SharedClientsContainer>();
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

pub(crate) fn command_parser_give() -> Command {
    Command::new("give".to_owned())
        .arg(Arg::new("player".to_owned()).required(true))
        .arg(Arg::new("item".to_owned()).required(true))
        .arg(Arg::new("amount".to_owned()).required(true))
}

pub(crate) fn command_give(
    world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    args: CommandMatch,
) -> Result<(), String> {
    let login = args.get_arg::<String, _>("player")?.clone();
    let item_slug = args.get_arg::<String, _>("item")?.clone();
    let amount = args.get_arg::<u16, _>("amount")?.clone();

    if amount == 0 {
        sender.send_console_message("&cAmount must be greater than zero".to_string());
        return Ok(());
    }

    let clients = world.resource::<SharedClientsContainer>();
    let items_manager = world.resource::<SharedItemsManager>();

    if !items_manager.read().has_item(&item_slug) {
        sender.send_console_message(format!("&cItem \"{}\" not found", item_slug));
        return Ok(());
    }

    let clients_guard = clients.read();
    let Some(client) = clients_guard.get_by_login(&login) else {
        sender.send_console_message(format!("&cPlayer with login \"{}\" not found", login));
        return Ok(());
    };

    let item = Item::create(item_slug.clone()).amount(amount);

    let result = client.with_player_data_mut(|player_data| {
        player_data.get_inventory_mut().add_item(item, |slot, updated_item| {
            send_inventory_changes_to_client(
                client,
                &crate::network::events::on_inventory_action::InventoryTarget::Client(client.get_client_id()),
                vec![network::messages::InventorySlotChange {
                    slot,
                    item: updated_item.map(|item| items_manager.read().to_client_item(item)),
                }],
            );
        })
    });

    match result {
        Some(Ok(())) => {
            sender.send_console_message(format!(
                "Admin &a{}&r gave &e{}x {}&r to &a{}&r",
                sender.get_name(),
                amount,
                item_slug,
                login
            ));
            Ok(())
        }
        Some(Err(_)) => {
            sender.send_console_message("&cPlayer inventory is full".to_string());
            Ok(())
        }
        None => {
            sender.send_console_message(format!("&cPlayer \"{}\" has no player data loaded", login));
            Ok(())
        }
    }
}
