use common::inventory::{inventory::Inventory, item::Item};
use network::messages::InventorySlotChange;

use crate::{
    clients::client::Client,
    clients::clients_container::SharedClientsContainer,
    inventory::inventory_manager::InventoryManager,
    items_manager::items_manager::SharedItemsManager,
    network::{
        events::on_inventory_action::{InventoryAction, InventoryTarget},
        sync_inventory::{
            broadcast_world_inventory_changes, send_inventory_changes_to_client, send_inventory_stop_to_client,
        },
    },
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
                        |inventory| {
                            move_within_inventory(
                                inventory,
                                items_manager,
                                from_slot as usize,
                                to_slot as usize,
                                amount,
                            )
                        },
                    );
                    broadcast_inventory_changes(client, clients, inventory_manager, from_inventory, changes);
                    return Ok(());
                }

                let Some((from_type, from_changes, to_type, to_changes)) = move_between_inventories(
                    client,
                    clients,
                    items_manager,
                    inventory_manager,
                    worlds_manager,
                    from_inventory.clone(),
                    from_slot as usize,
                    to_inventory.clone(),
                    to_slot as usize,
                    amount,
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
                if matches!(&inventory, InventoryTarget::Client(target_client_id) if *target_client_id == client.get_client_id())
                {
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

    fn authorize_action(
        client: &Client,
        action: &InventoryAction,
        inventory_manager: &InventoryManager,
    ) -> Result<(), String> {
        match action {
            InventoryAction::Move {
                from_inventory,
                to_inventory,
                ..
            } => {
                Self::authorize_inventory_target(client, inventory_manager, from_inventory)?;
                Self::authorize_inventory_target(client, inventory_manager, to_inventory)?;
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

fn calculate_transfer_amount(
    items_manager: &SharedItemsManager,
    source: &Item,
    target: Option<&Item>,
    requested: u16,
) -> u16 {
    if requested == 0 {
        return 0;
    }

    match target {
        Some(target) if target.can_stack_with(source) => {
            let max_stack_size = items_manager.read().get_max_stack_size(source);
            let space = max_stack_size.saturating_sub(target.get_amount());
            requested.min(space)
        }
        Some(_) => 0,
        None => requested,
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
            let chunk_column_arc = world
                .get_chunks_map()
                .get_chunk_column_arc(location.get_chunk_position())?;
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
            let chunk_column_arc = world
                .get_chunks_map()
                .get_chunk_column_arc(location.get_chunk_position())?;
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
    let changes = with_inventory_mut(
        client,
        clients,
        inventory_manager,
        worlds_manager,
        inventory_target,
        |inventory| {
            item = inventory.take_slot(slot);
            item.as_ref()
                .map(|_| vec![InventorySlotChange { slot, item: None }])
                .unwrap_or_default()
        },
    )?;
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
    with_inventory_mut(
        client,
        clients,
        inventory_manager,
        worlds_manager,
        inventory_target,
        |inventory| {
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
        },
    )?;
    Some((inventory_target.clone(), extracted?, source_change))
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
    let changes = with_inventory_mut(
        client,
        clients,
        inventory_manager,
        worlds_manager,
        inventory_target,
        |inventory| match inventory.get_slot(slot).cloned() {
            Some(existing) if existing.can_stack_with(&item) => {
                let max_stack_size = items_manager.read().get_max_stack_size(&item);
                let space = max_stack_size.saturating_sub(existing.get_amount());
                if space == 0 {
                    return Vec::new();
                }
                let added = item.get_amount().min(space);
                let updated = existing.clone().amount(existing.get_amount() + added);
                inventory.set_slot_option(slot, Some(updated.clone()));
                vec![InventorySlotChange {
                    slot,
                    item: Some(items_manager.read().to_client_item(&updated)),
                }]
            }
            None => {
                inventory.set_slot_option(slot, Some(item.clone()));
                vec![InventorySlotChange {
                    slot,
                    item: Some(items_manager.read().to_client_item(&item)),
                }]
            }
            _ => Vec::new(),
        },
    )?;
    if changes.is_empty() {
        return None;
    }
    Some((inventory_target.clone(), changes))
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

    let Some(source_preview) = inventory.get_slot(from_slot).cloned() else {
        return Vec::new();
    };

    let target_item = inventory.get_slot(to_slot).cloned();
    let transfer = amount.min(source_preview.get_amount());
    if transfer == 0 {
        return Vec::new();
    }

    if let Some(existing) = target_item.as_ref() {
        if !existing.can_stack_with(&source_preview) {
            if transfer != source_preview.get_amount() {
                return Vec::new();
            }

            let Some(source) = inventory.take_slot(from_slot) else {
                return Vec::new();
            };
            let Some(target) = inventory.take_slot(to_slot) else {
                inventory.set_slot_option(from_slot, Some(source));
                return Vec::new();
            };

            inventory.set_slot_option(from_slot, Some(target.clone()));
            inventory.set_slot_option(to_slot, Some(source.clone()));
            return vec![
                InventorySlotChange {
                    slot: from_slot,
                    item: inventory
                        .get_slot(from_slot)
                        .cloned()
                        .map(|item| items_manager.read().to_client_item(&item)),
                },
                InventorySlotChange {
                    slot: to_slot,
                    item: inventory
                        .get_slot(to_slot)
                        .cloned()
                        .map(|item| items_manager.read().to_client_item(&item)),
                },
            ];
        }
    }

    let accepted = calculate_transfer_amount(items_manager, &source_preview, target_item.as_ref(), transfer);
    if accepted == 0 {
        return Vec::new();
    }

    let Some(mut source) = inventory.take_slot(from_slot) else {
        return Vec::new();
    };

    let moved = source.clone().amount(accepted);
    let remaining = source.get_amount() - accepted;

    if remaining == 0 {
        inventory.set_slot_option(from_slot, None);
    } else {
        source = source.amount(remaining);
        inventory.set_slot_option(from_slot, Some(source));
    }

    let updated_target = match target_item {
        Some(existing) if existing.can_stack_with(&moved) => {
            let updated = existing.clone().amount(existing.get_amount() + accepted);
            inventory.set_slot_option(to_slot, Some(updated.clone()));
            Some(updated)
        }
        None => {
            inventory.set_slot_option(to_slot, Some(moved.clone()));
            Some(moved.clone())
        }
        _ => return Vec::new(),
    };

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
            item: updated_target.map(|item| items_manager.read().to_client_item(&item)),
        },
    ]
}

fn move_between_inventories(
    client: &Client,
    clients: &SharedClientsContainer,
    items_manager: &SharedItemsManager,
    inventory_manager: &InventoryManager,
    worlds_manager: &SharedWorldsManager,
    from_inventory: InventoryTarget,
    from_slot: usize,
    to_inventory: InventoryTarget,
    to_slot: usize,
    amount: u16,
) -> Option<(
    InventoryTarget,
    Vec<InventorySlotChange>,
    InventoryTarget,
    Vec<InventorySlotChange>,
)> {
    let source_item = with_inventory_ref(
        client,
        clients,
        inventory_manager,
        worlds_manager,
        &from_inventory,
        |inventory| inventory.get_slot(from_slot).cloned(),
    )
    .flatten()?;
    let target_item = with_inventory_ref(
        client,
        clients,
        inventory_manager,
        worlds_manager,
        &to_inventory,
        |inventory| inventory.get_slot(to_slot).cloned(),
    )
    .flatten();

    let requested = amount.min(source_item.get_amount());
    if requested == 0 {
        return None;
    }

    if let Some(existing) = target_item.as_ref() {
        if !existing.can_stack_with(&source_item) {
            if requested != source_item.get_amount() {
                return None;
            }

            return swap_between_inventories(
                client,
                clients,
                items_manager,
                inventory_manager,
                worlds_manager,
                from_inventory,
                from_slot,
                to_inventory,
                to_slot,
            );
        }
    }

    let accepted = calculate_transfer_amount(items_manager, &source_item, target_item.as_ref(), requested);
    if accepted == 0 {
        return None;
    }

    let Some((from_type, moved_item, from_changes)) = extract_item(
        client,
        clients,
        items_manager,
        inventory_manager,
        worlds_manager,
        &from_inventory,
        from_slot,
        accepted,
    ) else {
        return None;
    };

    let Some((to_type, to_changes)) = insert_item(
        client,
        clients,
        items_manager,
        inventory_manager,
        worlds_manager,
        &to_inventory,
        to_slot,
        moved_item.clone(),
    ) else {
        let restore_changes = with_inventory_mut(
            client,
            clients,
            inventory_manager,
            worlds_manager,
            &from_type,
            |inventory| {
                inventory.set_slot_option(from_slot, Some(source_item.clone()));
                vec![InventorySlotChange {
                    slot: from_slot,
                    item: Some(items_manager.read().to_client_item(&source_item)),
                }]
            },
        );
        broadcast_inventory_changes(client, clients, inventory_manager, from_type, restore_changes);
        return None;
    };

    Some((from_type, from_changes, to_type, to_changes))
}

fn swap_between_inventories(
    client: &Client,
    clients: &SharedClientsContainer,
    items_manager: &SharedItemsManager,
    inventory_manager: &InventoryManager,
    worlds_manager: &SharedWorldsManager,
    from_inventory: InventoryTarget,
    from_slot: usize,
    to_inventory: InventoryTarget,
    to_slot: usize,
) -> Option<(
    InventoryTarget,
    Vec<InventorySlotChange>,
    InventoryTarget,
    Vec<InventorySlotChange>,
)> {
    let Some((from_type, from_item, _)) = take_slot(
        client,
        clients,
        inventory_manager,
        worlds_manager,
        &from_inventory,
        from_slot,
    ) else {
        return None;
    };
    let Some((to_type, to_item, _)) = take_slot(
        client,
        clients,
        inventory_manager,
        worlds_manager,
        &to_inventory,
        to_slot,
    ) else {
        let _ = with_inventory_mut(
            client,
            clients,
            inventory_manager,
            worlds_manager,
            &from_type,
            |inventory| {
                inventory.set_slot_option(from_slot, Some(from_item.clone()));
            },
        );
        return None;
    };

    let _ = with_inventory_mut(
        client,
        clients,
        inventory_manager,
        worlds_manager,
        &from_type,
        |inventory| {
            inventory.set_slot_option(from_slot, Some(to_item.clone()));
        },
    );
    let _ = with_inventory_mut(
        client,
        clients,
        inventory_manager,
        worlds_manager,
        &to_type,
        |inventory| {
            inventory.set_slot_option(to_slot, Some(from_item.clone()));
        },
    );

    Some((
        from_type,
        vec![InventorySlotChange {
            slot: from_slot,
            item: Some(items_manager.read().to_client_item(&to_item)),
        }],
        to_type,
        vec![InventorySlotChange {
            slot: to_slot,
            item: Some(items_manager.read().to_client_item(&from_item)),
        }],
    ))
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
                    send_inventory_changes_to_client(
                        target_client,
                        &InventoryTarget::Client(target_client_id),
                        changes,
                    );
                }
            }
        }
        InventoryTarget::World(inventory_id) => {
            broadcast_world_inventory_changes(clients, inventory_manager, inventory_id, changes);
        }
    }
}
