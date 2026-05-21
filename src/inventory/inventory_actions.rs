use common::{
    inventory::{
        inventory::Inventory,
        item::Item,
    },
};
use network::messages::InventorySlotChange;

use crate::{
    clients::client::Client,
    clients::clients_container::SharedClientsContainer,
    items_manager::items_manager::SharedItemsManager,
    network::{
        events::on_inventory_action::{InventoryAction, InventoryTarget},
        sync_inventory::{
            broadcast_world_inventory_changes, send_inventory_changes_to_client, send_inventory_stop_to_client,
        },
    },
    inventory::inventory_manager::InventoryManager,
    worlds::worlds_manager::SharedWorldsManager,
};

pub struct InventoryActions;

impl InventoryActions {
    pub fn apply_action(
        client: &Client,
        action: InventoryAction,
        clients: &SharedClientsContainer,
        items_manager: &SharedItemsManager,
        inventory_manager: &mut InventoryManager,
        worlds_manager: &SharedWorldsManager,
    ) -> Result<(), String> {
        Self::authorize_action(client, &action, inventory_manager)?;

        match action {
            InventoryAction::Move {
                from_inventory,
                from_slot,
                to_inventory,
                to_slot,
                amount,
            } => {
                if from_inventory == to_inventory {
                    let changes = with_inventory_mut(
                        client,
                        clients,
                        inventory_manager,
                        worlds_manager,
                        &from_inventory,
                        |inventory| move_within_inventory(inventory, items_manager, from_slot as usize, to_slot as usize, amount),
                    );
                    broadcast_inventory_changes(client, clients, inventory_manager, from_inventory, changes);
                    return Ok(());
                }

                let source_ready = with_inventory_ref(
                    client,
                    clients,
                    inventory_manager,
                    worlds_manager,
                    &from_inventory,
                    |inventory| inventory.get_slot(from_slot as usize).is_some(),
                )
                .unwrap_or(false);
                if !source_ready {
                    return Ok(());
                }

                let target_ready = with_inventory_ref(
                    client,
                    clients,
                    inventory_manager,
                    worlds_manager,
                    &to_inventory,
                    |_| true,
                )
                .unwrap_or(false);
                if !target_ready {
                    return Ok(());
                }

                let Some((from_type, moved_item, from_changes)) = extract_item(
                    client,
                    clients,
                    items_manager,
                    inventory_manager,
                    worlds_manager,
                    &from_inventory,
                    from_slot as usize,
                    amount,
                ) else {
                    return Ok(());
                };

                let Some((to_type, to_changes)) = insert_item(
                    client,
                    clients,
                    items_manager,
                    inventory_manager,
                    worlds_manager,
                    &to_inventory,
                    to_slot as usize,
                    moved_item,
                ) else {
                    return Ok(());
                };

                broadcast_inventory_changes(client, clients, inventory_manager, from_type, Some(from_changes));
                broadcast_inventory_changes(client, clients, inventory_manager, to_type, Some(to_changes));
            }
            InventoryAction::Swap {
                a_inventory,
                a_slot,
                b_inventory,
                b_slot,
            } => {
                if a_inventory == b_inventory {
                    let changes = with_inventory_mut(
                        client,
                        clients,
                        inventory_manager,
                        worlds_manager,
                        &a_inventory,
                        |inventory| {
                            inventory.swap_slots(a_slot as usize, b_slot as usize);
                            vec![
                                InventorySlotChange {
                                    slot: a_slot as usize,
                                    item: Some({
                                        let item = inventory
                                            .get_slot(a_slot as usize)
                                            .expect("swap_slots must leave item in source slot");
                                        items_manager.read().to_client_item(item)
                                    }),
                                },
                                InventorySlotChange {
                                    slot: b_slot as usize,
                                    item: Some({
                                        let item = inventory
                                            .get_slot(b_slot as usize)
                                            .expect("swap_slots must leave item in target slot");
                                        items_manager.read().to_client_item(item)
                                    }),
                                },
                            ]
                        },
                    );
                    broadcast_inventory_changes(client, clients, inventory_manager, a_inventory, changes);
                    return Ok(());
                }

                let a_ready = with_inventory_ref(
                    client,
                    clients,
                    inventory_manager,
                    worlds_manager,
                    &a_inventory,
                    |inventory| inventory.get_slot(a_slot as usize).is_some(),
                )
                .unwrap_or(false);
                if !a_ready {
                    return Ok(());
                }

                let b_ready = with_inventory_ref(
                    client,
                    clients,
                    inventory_manager,
                    worlds_manager,
                    &b_inventory,
                    |inventory| inventory.get_slot(b_slot as usize).is_some(),
                )
                .unwrap_or(false);
                if !b_ready {
                    return Ok(());
                }

                let Some((a_type, a_item, a_changes)) = take_slot(
                    client,
                    clients,
                    inventory_manager,
                    worlds_manager,
                    &a_inventory,
                    a_slot as usize,
                ) else {
                    return Ok(());
                };
                let Some((b_type, b_item, b_changes)) = take_slot(
                    client,
                    clients,
                    inventory_manager,
                    worlds_manager,
                    &b_inventory,
                    b_slot as usize,
                ) else {
                    return Ok(());
                };

                let _ = with_inventory_mut(client, clients, inventory_manager, worlds_manager, &a_type, |inventory| {
                    inventory.set_slot_option(a_slot as usize, Some(b_item));
                });
                let _ = with_inventory_mut(client, clients, inventory_manager, worlds_manager, &b_type, |inventory| {
                    inventory.set_slot_option(b_slot as usize, Some(a_item));
                });

                broadcast_inventory_changes(client, clients, inventory_manager, a_type, Some(a_changes));
                broadcast_inventory_changes(client, clients, inventory_manager, b_type, Some(b_changes));
            }
            InventoryAction::Split {
                from_inventory,
                from_slot,
                to_inventory,
                to_slot,
                amount,
            } => {
                if from_inventory == to_inventory {
                    let changes = with_inventory_mut(
                        client,
                        clients,
                        inventory_manager,
                        worlds_manager,
                        &from_inventory,
                        |inventory| split_within_inventory(inventory, items_manager, from_slot as usize, to_slot as usize, amount),
                    );
                    broadcast_inventory_changes(client, clients, inventory_manager, from_inventory, changes);
                    return Ok(());
                }

                let source_ready = with_inventory_ref(
                    client,
                    clients,
                    inventory_manager,
                    worlds_manager,
                    &from_inventory,
                    |inventory| inventory.get_slot(from_slot as usize).is_some(),
                )
                .unwrap_or(false);
                if !source_ready {
                    return Ok(());
                }

                let target_ready = with_inventory_ref(
                    client,
                    clients,
                    inventory_manager,
                    worlds_manager,
                    &to_inventory,
                    |_| true,
                )
                .unwrap_or(false);
                if !target_ready {
                    return Ok(());
                }

                let Some((from_type, moved_item, from_changes)) = split_out(
                    client,
                    clients,
                    items_manager,
                    inventory_manager,
                    worlds_manager,
                    &from_inventory,
                    from_slot as usize,
                    amount,
                ) else {
                    return Ok(());
                };

                let Some((to_type, to_changes)) = insert_split(
                    client,
                    clients,
                    items_manager,
                    inventory_manager,
                    worlds_manager,
                    &to_inventory,
                    to_slot as usize,
                    moved_item,
                ) else {
                    return Ok(());
                };

                broadcast_inventory_changes(client, clients, inventory_manager, from_type, Some(from_changes));
                broadcast_inventory_changes(client, clients, inventory_manager, to_type, Some(to_changes));
            }
            InventoryAction::Drop {
                inventory,
                slot,
                amount,
            } => {
                let changes = with_inventory_mut(
                    client,
                    clients,
                    inventory_manager,
                    worlds_manager,
                    &inventory,
                    |inventory_data| drop_stack(inventory_data, items_manager, slot as usize, amount),
                );
                broadcast_inventory_changes(client, clients, inventory_manager, inventory, changes);
            }
            InventoryAction::Close { inventory } => {
                if matches!(&inventory, InventoryTarget::Client(target_client_id) if *target_client_id == client.get_client_id()) {
                    log::error!(
                        target: "inventory",
                        "client {} tried to close own inventory",
                        client.get_client_id()
                    );
                    return Ok(());
                }

                let Some(world_entity) = client.get_world_entity() else {
                    log::error!(
                        target: "inventory",
                        "client {} tried to close inventory without world entity",
                        client.get_client_id()
                    );
                    return Ok(());
                };

                match &inventory {
                    InventoryTarget::Client(target_client_id) => {
                        inventory_manager.close_inventory(world_entity.get_entity(), *target_client_id);
                    }
                    InventoryTarget::World(inventory_id) => {
                        inventory_manager.close_inventory(world_entity.get_entity(), *inventory_id);
                    }
                }

                send_inventory_stop_to_client(client, &inventory);
            }
        }
        Ok(())
    }

    fn authorize_action(client: &Client, action: &InventoryAction, inventory_manager: &InventoryManager) -> Result<(), String> {
        match action {
            InventoryAction::Move {
                from_inventory,
                to_inventory,
                ..
            }
            | InventoryAction::Split {
                from_inventory,
                to_inventory,
                ..
            } => {
                Self::authorize_inventory_target(client, inventory_manager, from_inventory)?;
                Self::authorize_inventory_target(client, inventory_manager, to_inventory)?;
            }
            InventoryAction::Swap {
                a_inventory,
                b_inventory,
                ..
            } => {
                Self::authorize_inventory_target(client, inventory_manager, a_inventory)?;
                Self::authorize_inventory_target(client, inventory_manager, b_inventory)?;
            }
            InventoryAction::Drop { inventory, .. } | InventoryAction::Close { inventory } => {
                Self::authorize_inventory_target(client, inventory_manager, inventory)?;
            }
        }

        Ok(())
    }

    fn authorize_inventory_target(
        client: &Client,
        inventory_manager: &InventoryManager,
        inventory_target: &InventoryTarget,
    ) -> Result<(), String> {
        match inventory_target {
            InventoryTarget::Client(target_client_id) if *target_client_id == client.get_client_id() => Ok(()),
            InventoryTarget::Client(target_client_id) => Err(format!(
                "client {} is not allowed to act on client {} inventory",
                client.get_client_id(),
                target_client_id
            )),
            InventoryTarget::World(inventory_id) => {
                let Some(world_entity) = client.get_world_entity() else {
                    return Err(format!(
                        "client {} tried to act on world inventory {} without world entity",
                        client.get_client_id(),
                        inventory_id
                    ));
                };

                let Some(watchers) = inventory_manager.state().get_inventory_watchers(inventory_id) else {
                    return Err(format!("inventory {} is not watched", inventory_id));
                };

                if !watchers.iter().any(|watcher| *watcher == world_entity.get_entity()) {
                    return Err(format!(
                        "client {} is not watching inventory {}",
                        client.get_client_id(),
                        inventory_id
                    ));
                }
                Ok(())
            }
        }
    }
}

fn with_inventory_mut<R>(
    client: &Client,
    clients: &SharedClientsContainer,
    inventory_manager: &InventoryManager,
    worlds_manager: &SharedWorldsManager,
    inventory_target: &InventoryTarget,
    f: impl FnOnce(&mut Inventory) -> R,
) -> Option<R> {
    match inventory_target {
        InventoryTarget::Client(target_client_id) if *target_client_id == client.get_client_id() => {
            client.with_player_data_mut(|player_data| f(player_data.get_inventory_mut()))
        }
        InventoryTarget::Client(target_client_id) => {
            let clients = clients.read();
            let target_client = clients.get(target_client_id)?;
            target_client.with_player_data_mut(|player_data| f(player_data.get_inventory_mut()))
        }
        InventoryTarget::World(inventory_id) => {
            let location = inventory_manager.state().get_inventory_location(inventory_id)?.clone();
            let worlds = worlds_manager.read();
            let world = worlds.get_world_manager(location.get_world_slug())?;
            let chunk_column_arc = world.get_chunks_map().get_chunk_column_arc(location.get_chunk_position())?;
            let mut chunk_column = chunk_column_arc.write();
            let chunk_storage = chunk_column.get_chunk_storage_mut();
            let block_inventory = chunk_storage.get_inventory_mut(*inventory_id)?;
            Some(f(block_inventory.get_inventory_mut()))
        }
    }
}

fn with_inventory_ref<R>(
    client: &Client,
    clients: &SharedClientsContainer,
    inventory_manager: &InventoryManager,
    worlds_manager: &SharedWorldsManager,
    inventory_target: &InventoryTarget,
    f: impl FnOnce(&Inventory) -> R,
) -> Option<R> {
    match inventory_target {
        InventoryTarget::Client(target_client_id) if *target_client_id == client.get_client_id() => {
            client.with_player_data_mut(|player_data| f(player_data.get_inventory_mut()))
        }
        InventoryTarget::Client(target_client_id) => {
            let clients = clients.read();
            let target_client = clients.get(target_client_id)?;
            target_client.with_player_data_mut(|player_data| f(player_data.get_inventory_mut()))
        }
        InventoryTarget::World(inventory_id) => {
            let location = inventory_manager.state().get_inventory_location(inventory_id)?.clone();
            let worlds = worlds_manager.read();
            let world = worlds.get_world_manager(location.get_world_slug())?;
            let chunk_column_arc = world.get_chunks_map().get_chunk_column_arc(location.get_chunk_position())?;
            let mut chunk_column = chunk_column_arc.write();
            let chunk_storage = chunk_column.get_chunk_storage_mut();
            let block_inventory = chunk_storage.get_inventory_mut(*inventory_id)?;
            Some(f(block_inventory.get_inventory_mut()))
        }
    }
}

fn take_slot(
    client: &Client,
    clients: &SharedClientsContainer,
    inventory_manager: &InventoryManager,
    worlds_manager: &SharedWorldsManager,
    inventory_target: &InventoryTarget,
    slot: usize,
) -> Option<(InventoryTarget, Item, Vec<InventorySlotChange>)> {
    let mut item = None;
    let changes = with_inventory_mut(client, clients, inventory_manager, worlds_manager, inventory_target, |inventory| {
        item = inventory.take_slot(slot);
        item.as_ref().map(|_| vec![InventorySlotChange { slot, item: None }]).unwrap_or_default()
    })?;
    Some((inventory_target.clone(), item?, changes))
}

fn extract_item(
    client: &Client,
    clients: &SharedClientsContainer,
    items_manager: &SharedItemsManager,
    inventory_manager: &InventoryManager,
    worlds_manager: &SharedWorldsManager,
    inventory_target: &InventoryTarget,
    slot: usize,
    amount: u16,
) -> Option<(InventoryTarget, Item, Vec<InventorySlotChange>)> {
    let mut extracted = None;
    let mut source_change = Vec::new();
    with_inventory_mut(client, clients, inventory_manager, worlds_manager, inventory_target, |inventory| {
        let Some(mut item) = inventory.take_slot(slot) else {
            return;
        };
        let transfer = amount.min(item.get_amount());
        if transfer == 0 {
            inventory.set_slot_option(slot, Some(item));
            return;
        }
        let remaining = item.get_amount() - transfer;
        let moved = item.clone().amount(transfer);
        if remaining == 0 {
            source_change = vec![InventorySlotChange { slot, item: None }];
        } else {
            item = item.amount(remaining);
            inventory.set_slot_option(slot, Some(item));
            source_change = vec![InventorySlotChange {
                slot,
                item: inventory
                    .get_slot(slot)
                    .cloned()
                    .map(|item| items_manager.read().to_client_item(&item)),
            }];
        }
        extracted = Some(moved);
    })?;
    Some((inventory_target.clone(), extracted?, source_change))
}

fn split_out(
    client: &Client,
    clients: &SharedClientsContainer,
    items_manager: &SharedItemsManager,
    inventory_manager: &InventoryManager,
    worlds_manager: &SharedWorldsManager,
    inventory_target: &InventoryTarget,
    slot: usize,
    amount: u16,
) -> Option<(InventoryTarget, Item, Vec<InventorySlotChange>)> {
    extract_item(client, clients, items_manager, inventory_manager, worlds_manager, inventory_target, slot, amount)
}

fn insert_item(
    client: &Client,
    clients: &SharedClientsContainer,
    items_manager: &SharedItemsManager,
    inventory_manager: &InventoryManager,
    worlds_manager: &SharedWorldsManager,
    inventory_target: &InventoryTarget,
    slot: usize,
    item: Item,
) -> Option<(InventoryTarget, Vec<InventorySlotChange>)> {
    let changes = with_inventory_mut(client, clients, inventory_manager, worlds_manager, inventory_target, |inventory| {
        inventory.set_slot_option(slot, Some(item.clone()));
        vec![InventorySlotChange {
            slot,
            item: Some(items_manager.read().to_client_item(&item)),
        }]
    })?;
    Some((inventory_target.clone(), changes))
}

fn insert_split(
    client: &Client,
    clients: &SharedClientsContainer,
    items_manager: &SharedItemsManager,
    inventory_manager: &InventoryManager,
    worlds_manager: &SharedWorldsManager,
    inventory_target: &InventoryTarget,
    slot: usize,
    item: Item,
) -> Option<(InventoryTarget, Vec<InventorySlotChange>)> {
    insert_item(client, clients, items_manager, inventory_manager, worlds_manager, inventory_target, slot, item)
}

fn move_within_inventory(
    inventory: &mut Inventory,
    items_manager: &SharedItemsManager,
    from_slot: usize,
    to_slot: usize,
    amount: u16,
) -> Vec<InventorySlotChange> {
    if amount == 0 || from_slot == to_slot {
        return Vec::new();
    }

    let Some(mut source) = inventory.take_slot(from_slot) else {
        return Vec::new();
    };

    let transfer = amount.min(source.get_amount());
    if transfer == 0 {
        inventory.set_slot_option(from_slot, Some(source));
        return Vec::new();
    }

    let moved = source.clone().amount(transfer);
    let remaining = source.get_amount() - transfer;

    if remaining == 0 {
        inventory.set_slot_option(from_slot, None);
    } else {
        source = source.amount(remaining);
        inventory.set_slot_option(from_slot, Some(source));
    }

    inventory.set_slot_option(to_slot, Some(moved.clone()));

    vec![
        InventorySlotChange {
            slot: from_slot,
            item: inventory
                .get_slot(from_slot)
                .cloned()
                .map(|item| items_manager.read().to_client_item(&item)),
        },
        InventorySlotChange {
            slot: to_slot,
            item: Some(items_manager.read().to_client_item(&moved)),
        },
    ]
}

fn split_within_inventory(
    inventory: &mut Inventory,
    items_manager: &SharedItemsManager,
    from_slot: usize,
    to_slot: usize,
    amount: u16,
) -> Vec<InventorySlotChange> {
    if amount == 0 || from_slot == to_slot {
        return Vec::new();
    }

    let Some(mut source) = inventory.take_slot(from_slot) else {
        return Vec::new();
    };

    let transfer = amount.min(source.get_amount());
    if transfer == 0 {
        inventory.set_slot_option(from_slot, Some(source));
        return Vec::new();
    }

    let moved = source.clone().amount(transfer);
    let remaining = source.get_amount() - transfer;
    source = source.amount(remaining);

    if source.get_amount() == 0 {
        inventory.set_slot_option(from_slot, None);
    } else {
        inventory.set_slot_option(from_slot, Some(source));
    }

    inventory.set_slot_option(to_slot, Some(moved.clone()));
    vec![
        InventorySlotChange {
            slot: from_slot,
            item: inventory
                .get_slot(from_slot)
                .cloned()
                .map(|item| items_manager.read().to_client_item(&item)),
        },
        InventorySlotChange {
            slot: to_slot,
            item: Some(items_manager.read().to_client_item(&moved)),
        },
    ]
}

fn drop_stack(
    inventory: &mut Inventory,
    items_manager: &SharedItemsManager,
    slot: usize,
    amount: u16,
) -> Vec<InventorySlotChange> {
    if amount == 0 {
        return Vec::new();
    }

    let Some(mut source) = inventory.take_slot(slot) else {
        return Vec::new();
    };

    let removed_amount = amount.min(source.get_amount());
    if removed_amount == 0 {
        inventory.set_slot_option(slot, Some(source));
        return Vec::new();
    }

    if removed_amount < source.get_amount() {
        let remaining = source.get_amount() - removed_amount;
        source = source.amount(remaining);
        inventory.set_slot_option(slot, Some(source.clone()));
        vec![InventorySlotChange {
            slot,
            item: Some(items_manager.read().to_client_item(&source)),
        }]
    } else {
        vec![InventorySlotChange { slot, item: None }]
    }
}

fn broadcast_inventory_changes(
    client: &Client,
    clients: &SharedClientsContainer,
    inventory_manager: &InventoryManager,
    inventory_target: InventoryTarget,
    changes: Option<Vec<InventorySlotChange>>,
) {
    let Some(changes) = changes else {
        return;
    };
    if changes.is_empty() {
        return;
    }

    match inventory_target {
        InventoryTarget::Client(target_client_id) => {
            send_inventory_changes_to_client(client, &InventoryTarget::Client(target_client_id), changes.clone());
            if let Some(target_client) = clients.read().get(&target_client_id) {
                if target_client.get_client_id() != client.get_client_id() {
                    send_inventory_changes_to_client(target_client, &InventoryTarget::Client(target_client_id), changes);
                }
            }
        }
        InventoryTarget::World(inventory_id) => {
            broadcast_world_inventory_changes(clients, inventory_manager, inventory_id, changes);
        }
    }
}
