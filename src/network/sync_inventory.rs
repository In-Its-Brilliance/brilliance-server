use common::inventory::inventory::{ClientInventory, InventoryType, InventoryType::*};
use network::messages::{InventorySlotChange, InventoryStream, NetworkMessageType, ServerMessages};

use crate::{
    clients::client::Client, clients::clients_container::SharedClientsContainer,
    inventory::inventory_manager::InventoryManager, network::events::on_inventory_action::InventoryTarget,
};

pub fn send_inventory_stream(client: &Client, stream: InventoryStream) {
    let msg = ServerMessages::InventoryStream(stream);
    client.send_message(NetworkMessageType::ReliableOrdered, &msg);
}

pub fn send_inventory_start_to_client(client: &Client, inventory_type: InventoryType, inventory: ClientInventory) {
    send_inventory_stream(
        client,
        InventoryStream::StartStream {
            inventory_type,
            inventory,
        },
    );
}

pub fn send_inventory_stop_to_client(client: &Client, target: &InventoryTarget) {
    let stream = InventoryStream::StopStream {
        inventory_type: inventory_type_for_recipient(target, client.get_client_id()),
    };
    send_inventory_stream(client, stream);
}

/// Convert server-side target into the client's view of the same inventory.
fn inventory_type_for_recipient(target: &InventoryTarget, recipient_client_id: u64) -> InventoryType {
    match target {
        InventoryTarget::Client(client_id) if *client_id == recipient_client_id => PlayerPersonal,
        InventoryTarget::Client(client_id) => OtherPlayer(*client_id),
        InventoryTarget::World(inventory_id) => WorldInventory(*inventory_id),
    }
}

pub fn send_inventory_changes_to_client(client: &Client, target: &InventoryTarget, changes: Vec<InventorySlotChange>) {
    let stream = InventoryStream::UpdateSlots {
        inventory_type: inventory_type_for_recipient(target, client.get_client_id()),
        changes,
    };
    send_inventory_stream(client, stream);
}

pub fn broadcast_world_inventory_changes(
    clients: &SharedClientsContainer,
    inventory_manager: &InventoryManager,
    inventory_id: u64,
    changes: Vec<InventorySlotChange>,
) {
    if changes.is_empty() {
        return;
    }

    let Some(watchers) = inventory_manager.state().get_inventory_watchers(&inventory_id) else {
        return;
    };
    let stream = InventoryStream::UpdateSlots {
        inventory_type: WorldInventory(inventory_id),
        changes,
    };

    let clients_guard = clients.read();
    for watcher in watchers {
        let Some(watcher_client) = clients_guard.iter().find_map(|(_, candidate)| {
            let Some(world_entity) = candidate.get_world_entity() else {
                return None;
            };
            (world_entity.get_entity() == *watcher).then_some(candidate)
        }) else {
            continue;
        };
        send_inventory_stream(watcher_client, stream.clone());
    }
}
