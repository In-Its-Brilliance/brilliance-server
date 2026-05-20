use bevy_ecs::world::World;
use common::commands::command::{Arg, ArgCompleterContext, Command, CommandMatch};
use common::inventory::item::Item;

use crate::{
    clients::clients_container::SharedClientsContainer,
    console::console_sender::ConsoleSenderType,
    items_manager::items_manager::SharedItemsManager,
    network::sync_inventory::send_inventory_changes_to_client,
};

fn world_from_context(context: &dyn ArgCompleterContext) -> &World {
    let Some(world) = context.world().downcast_ref::<World>() else {
        panic!("ArgCompleterContext world is not bevy_ecs::world::World");
    };
    world
}

pub(crate) fn complete_players(context: &dyn ArgCompleterContext, input: &str) -> Vec<String> {
    let world = world_from_context(context);
    let Some(clients) = world.get_resource::<SharedClientsContainer>() else {
        return Vec::new();
    };

    let clients_guard = clients.read();
    let mut logins: Vec<String> = clients_guard
        .iter()
        .filter_map(|(_, client)| client.get_client_info().map(|info| info.get_login().to_string()))
        .filter(|login| login.contains(input))
        .collect();
    logins.sort();
    logins
}

pub(crate) fn complete_items(context: &dyn ArgCompleterContext, input: &str) -> Vec<String> {
    let world = world_from_context(context);
    let Some(items_manager) = world.get_resource::<SharedItemsManager>() else {
        return Vec::new();
    };

    let mut slugs: Vec<String> = items_manager
        .read()
        .iter_slugs()
        .filter(|slug| slug.contains(input))
        .cloned()
        .collect();
    slugs.sort();
    slugs
}

pub(crate) fn command_parser_give() -> Command {
    Command::new("give".to_owned())
        .arg(Arg::new("player".to_owned()).required(true).completer(complete_players))
        .arg(Arg::new("item".to_owned()).required(true).completer(complete_items))
        .arg(Arg::new("amount".to_owned()).required(false))
}

pub(crate) fn command_parser_kick() -> Command {
    Command::new("kick".to_owned())
        .arg(Arg::new("player".to_owned()).required(true).completer(complete_players))
        .arg(Arg::new("message".to_owned()).required(false))
}

pub(crate) fn command_parser_clear() -> Command {
    Command::new("clear".to_owned()).arg(Arg::new("player".to_owned()).required(true).completer(complete_players))
}

fn clear_player_inventory(client: &crate::clients::client::Client) -> usize {
    client
        .with_player_data_mut(|player_data| {
            let inventory = player_data.get_inventory_mut();
            let slots_len = inventory.slots_len();
            inventory.clear();
            slots_len
        })
        .unwrap_or(0)
}

pub(crate) fn command_give(
    world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    args: CommandMatch,
) -> Result<(), String> {
    let login = args.get_arg::<String, _>("player")?.clone();
    let item_slug = args.get_arg::<String, _>("item")?.clone();
    let amount = match args.get_arg::<u16, _>("amount") {
        Ok(value) => value,
        Err(_) => 1,
    };

    if amount == 0 {
        sender.send_console_message("&cAmount must be greater than zero".to_string());
        return Ok(());
    }

    let Some(clients) = world.get_resource::<SharedClientsContainer>() else {
        sender.send_console_message("&cClients container is not loaded".to_string());
        return Ok(());
    };
    let Some(items_manager) = world.get_resource::<SharedItemsManager>() else {
        sender.send_console_message("&cItems manager is not loaded".to_string());
        return Ok(());
    };

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

pub(crate) fn command_kick(
    world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    args: CommandMatch,
) -> Result<(), String> {
    let login = args.get_arg::<String, _>("player")?.clone();

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
    Ok(())
}

pub(crate) fn command_clear(
    world: &mut World,
    sender: Box<dyn ConsoleSenderType>,
    args: CommandMatch,
) -> Result<(), String> {
    let login = args.get_arg::<String, _>("player")?.clone();

    let Some(clients) = world.get_resource::<SharedClientsContainer>() else {
        sender.send_console_message("&cClients container is not loaded".to_string());
        return Ok(());
    };

    let clients_guard = clients.read();
    let Some(client) = clients_guard.get_by_login(&login) else {
        sender.send_console_message(format!("&cPlayer with login \"{}\" not found", login));
        return Ok(());
    };

    let slots_len = clear_player_inventory(client);
    if slots_len == 0 {
        sender.send_console_message(format!("&cPlayer \"{}\" has no player data loaded", login));
        return Ok(());
    }

    let changes: Vec<network::messages::InventorySlotChange> = (0..slots_len)
        .map(|slot| network::messages::InventorySlotChange { slot, item: None })
        .collect();
    send_inventory_changes_to_client(
        client,
        &crate::network::events::on_inventory_action::InventoryTarget::Client(client.get_client_id()),
        changes,
    );

    sender.send_console_message(format!("Admin &a{}&r cleared inventory of &a{}&r", sender.get_name(), login));
    Ok(())
}
