use common::inventory::{inventory::Inventory, item::Item};
use network::messages::InventorySlotChange;

use crate::{
    clients::client::Client,
    clients::clients_container::SharedClientsContainer,
    inventory::inventory_manager::InventoryManager,
    items_manager::items_manager::SharedItemsManager,
    network::{
        events::on_inventory_action::InventoryTarget,
        sync_inventory::{broadcast_world_inventory_changes, send_inventory_changes_to_client},
    },
    worlds::worlds_manager::SharedWorldsManager,
};

pub(crate) struct InventoryActionCtx<'a> {
    pub(crate) client: &'a Client,
    pub(crate) clients: &'a SharedClientsContainer,
    pub(crate) items_manager: &'a SharedItemsManager,
    pub(crate) worlds_manager: &'a SharedWorldsManager,
}

pub(crate) fn calculate_transfer_amount(
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

pub(crate) fn with_inventory_mut<R>(
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &InventoryManager,
    inventory_target: &InventoryTarget,
    f: impl FnOnce(&mut Inventory) -> R,
) -> Option<R> {
    match inventory_target {
        InventoryTarget::Client(target_client_id) if *target_client_id == ctx.client.get_client_id() => ctx
            .client
            .with_player_data_mut(|player_data| f(player_data.get_inventory_mut())),
        InventoryTarget::Client(target_client_id) => {
            let clients = ctx.clients.read();
            let target_client = clients.get(target_client_id)?;
            target_client.with_player_data_mut(|player_data| f(player_data.get_inventory_mut()))
        }
        InventoryTarget::World(inventory_id) => {
            let location = inventory_manager.state().get_inventory_location(inventory_id)?.clone();
            let worlds = ctx.worlds_manager.read();
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

pub(crate) fn with_inventory_ref<R>(
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &InventoryManager,
    inventory_target: &InventoryTarget,
    f: impl FnOnce(&Inventory) -> R,
) -> Option<R> {
    match inventory_target {
        InventoryTarget::Client(target_client_id) if *target_client_id == ctx.client.get_client_id() => ctx
            .client
            .with_player_data_mut(|player_data| f(player_data.get_inventory_mut())),
        InventoryTarget::Client(target_client_id) => {
            let clients = ctx.clients.read();
            let target_client = clients.get(target_client_id)?;
            target_client.with_player_data_mut(|player_data| f(player_data.get_inventory_mut()))
        }
        InventoryTarget::World(inventory_id) => {
            let location = inventory_manager.state().get_inventory_location(inventory_id)?.clone();
            let worlds = ctx.worlds_manager.read();
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

pub(crate) fn broadcast_inventory_changes(
    ctx: &InventoryActionCtx<'_>,
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
            send_inventory_changes_to_client(ctx.client, &InventoryTarget::Client(target_client_id), changes.clone());
            if let Some(target_client) = ctx.clients.read().get(&target_client_id) {
                if target_client.get_client_id() != ctx.client.get_client_id() {
                    send_inventory_changes_to_client(
                        target_client,
                        &InventoryTarget::Client(target_client_id),
                        changes,
                    );
                }
            }
        }
        InventoryTarget::World(inventory_id) => {
            broadcast_world_inventory_changes(ctx.clients, inventory_manager, inventory_id, changes);
        }
    }
}
