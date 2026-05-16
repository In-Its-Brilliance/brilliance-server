use common::inventory::inventory::InventoryType;
use common::utils::debug::SmartRwLock;
use std::sync::Arc;

use crate::{
    clients::clients_container::ClientsContainer,
    inventory::inventory_manager::InventoryManager,
    network::sync_inventory::{send_inventory_start_to_client, send_inventory_stop_to_client},
    worlds::worlds_manager::WorldsManager,
};

pub fn open_inventory(
    client_id: u64,
    inventory_id: u64,
    clients: &Arc<SmartRwLock<ClientsContainer>>,
    inventory_manager: &Arc<SmartRwLock<InventoryManager>>,
    worlds_manager: &Arc<SmartRwLock<WorldsManager>>,
) {
    let clients_guard = clients.read();
    let Some(client) = clients_guard.get(&client_id) else {
        log::error!(target: "inventory", "client {} not found for open_inventory", client_id);
        return;
    };
    let Some(world_entity) = client.get_world_entity() else {
        log::error!(target: "inventory", "client {} is not in a world", client_id);
        return;
    };
    let Some(player_data) = client.get_player_data() else {
        log::error!(target: "inventory", "player data is not loaded for client {}", client_id);
        return;
    };
    if player_data.get_inventory().get_id() == inventory_id {
        log::error!(target: "inventory", "client {} tried to open own inventory", client_id);
        return;
    }

    let inventory_manager_guard = inventory_manager.read();
    let Some(location) = inventory_manager_guard.state().get_inventory_location(&inventory_id).cloned() else {
        log::error!(target: "inventory", "inventory {} is not registered", inventory_id);
        return;
    };
    drop(inventory_manager_guard);

    let worlds_guard = worlds_manager.read();
    let Some(world_manager) = worlds_guard.get_world_manager(location.get_world_slug()) else {
        log::error!(target: "inventory", "world {} not found", location.get_world_slug());
        return;
    };
    let Some(chunk_column_arc) = world_manager.get_chunks_map().get_chunk_column_arc(location.get_chunk_position()) else {
        log::error!(target: "inventory", "chunk {:?} is not loaded", location.get_chunk_position());
        return;
    };
    let chunk_column = chunk_column_arc.read();
    let Some(block_inventory) = chunk_column
        .get_chunk_storage()
        .get_inventories()
        .iter()
        .find(|block_inventory| block_inventory.get_inventory().get_id() == inventory_id)
    else {
        log::error!(target: "inventory", "inventory {} not found in chunk storage", inventory_id);
        return;
    };

    let opened = inventory_manager
        .write()
        .open_inventory(world_entity.get_entity(), inventory_id);
    if opened {
        send_inventory_start_to_client(
            client,
            InventoryType::WorldInventory(inventory_id),
            block_inventory.get_inventory().to_client_inventory(),
        );
    }
}

pub fn close_inventory(
    client_id: u64,
    inventory_id: u64,
    clients: &Arc<SmartRwLock<ClientsContainer>>,
    inventory_manager: &Arc<SmartRwLock<InventoryManager>>,
) {
    let clients_guard = clients.read();
    let Some(client) = clients_guard.get(&client_id) else {
        log::error!(target: "inventory", "client {} not found for close_inventory", client_id);
        return;
    };
    let Some(world_entity) = client.get_world_entity() else {
        log::error!(target: "inventory", "client {} is not in a world", client_id);
        return;
    };
    let Some(player_data) = client.get_player_data() else {
        log::error!(target: "inventory", "player data is not loaded for client {}", client_id);
        return;
    };
    if player_data.get_inventory().get_id() == inventory_id {
        log::error!(target: "inventory", "client {} tried to close own inventory", client_id);
        return;
    }

    inventory_manager
        .write()
        .close_inventory(world_entity.get_entity(), inventory_id);
    send_inventory_stop_to_client(client, &crate::network::events::on_inventory_action::InventoryTarget::World(inventory_id));
}

pub fn get_or_create_inventory(
    world_slug: String,
    position: common::chunks::block_position::BlockPosition,
    slots_count: usize,
    worlds_manager: &Arc<SmartRwLock<WorldsManager>>,
    inventory_manager: &Arc<SmartRwLock<InventoryManager>>,
) -> Result<u64, String> {
    use common::chunks::block_position::BlockPositionTrait;

    let (section, block_position) = position.get_block_position();
    let chunk_position = position.get_chunk_position();

    let worlds_guard = worlds_manager.write();
    let Some(world_manager) = worlds_guard.get_world_manager_mut(&world_slug) else {
        return Err(format!("World \"{}\" not found", world_slug));
    };

    let Some(chunk_column_arc) = world_manager.get_chunks_map().get_chunk_column_arc(&chunk_position) else {
        return Err(format!("Chunk {:?} is not loaded", chunk_position));
    };

    let mut chunk_column = chunk_column_arc.write();
    let chunk_storage = chunk_column.get_chunk_storage_mut();
    let inventory_id = rand::random::<u64>();
    let block_inventory = chunk_storage.get_or_create_inventory_by_position_mut(
        section,
        block_position,
        slots_count,
        inventory_id,
    );

    let mut inventory_manager = inventory_manager.write();
    if inventory_manager.state().get_inventory_location(&inventory_id).is_none() {
        inventory_manager
            .state_mut()
            .register_world_inventory(world_slug, chunk_position, block_inventory);
    }

    Ok(inventory_id)
}
