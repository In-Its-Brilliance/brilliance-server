use bevy_ecs::message::Message;
use bevy_ecs::system::Res;
use common::inventory::inventory::InventoryType;
use common::utils::events::EventReader;
use network::messages::InventoryAction as ClientInventoryAction;

use crate::{
    clients::client::Client,
    clients::clients_container::SharedClientsContainer,
    inventory::SharedInventoryManager,
    network::server::NetworkEventListener,
    worlds::worlds_manager::SharedWorldsManager,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InventoryTarget {
    Client(u64),
    World(u64),
}

#[derive(Clone, Debug)]
pub enum InventoryAction {
    Move {
        from_inventory: InventoryTarget,
        from_slot: u16,
        to_inventory: InventoryTarget,
        to_slot: u16,
        amount: u16,
    },
    Swap {
        a_inventory: InventoryTarget,
        a_slot: u16,
        b_inventory: InventoryTarget,
        b_slot: u16,
    },
    Split {
        from_inventory: InventoryTarget,
        from_slot: u16,
        to_inventory: InventoryTarget,
        to_slot: u16,
        amount: u16,
    },
    Drop {
        inventory: InventoryTarget,
        slot: u16,
        amount: u16,
    },
    Close {
        inventory: InventoryTarget,
    },
}

impl InventoryAction {
    pub fn from_client_action(client_id: u64, action: ClientInventoryAction) -> Self {
        fn map_target(client_id: u64, inventory_type: InventoryType) -> InventoryTarget {
            match inventory_type {
                InventoryType::PlayerPersonal => InventoryTarget::Client(client_id),
                InventoryType::OtherPlayer(other_client_id) => InventoryTarget::Client(other_client_id),
                InventoryType::WorldInventory(inventory_id) => InventoryTarget::World(inventory_id),
            }
        }

        match action {
            ClientInventoryAction::Move {
                from_inventory,
                from_slot,
                to_inventory,
                to_slot,
                amount,
            } => Self::Move {
                from_inventory: map_target(client_id, from_inventory),
                from_slot,
                to_inventory: map_target(client_id, to_inventory),
                to_slot,
                amount,
            },
            ClientInventoryAction::Swap {
                a_inventory,
                a_slot,
                b_inventory,
                b_slot,
            } => Self::Swap {
                a_inventory: map_target(client_id, a_inventory),
                a_slot,
                b_inventory: map_target(client_id, b_inventory),
                b_slot,
            },
            ClientInventoryAction::Split {
                from_inventory,
                from_slot,
                to_inventory,
                to_slot,
                amount,
            } => Self::Split {
                from_inventory: map_target(client_id, from_inventory),
                from_slot,
                to_inventory: map_target(client_id, to_inventory),
                to_slot,
                amount,
            },
            ClientInventoryAction::Drop {
                inventory,
                slot,
                amount,
            } => Self::Drop {
                inventory: map_target(client_id, inventory),
                slot,
                amount,
            },
            ClientInventoryAction::Close { inventory } => Self::Close {
                inventory: map_target(client_id, inventory),
            },
        }
    }
}

#[derive(Message)]
pub struct InventoryActionEvent {
    client: Client,
    inventory_action: ClientInventoryAction,
}

impl InventoryActionEvent {
    pub fn new(client: Client, inventory_action: ClientInventoryAction) -> Self {
        Self {
            client,
            inventory_action,
        }
    }

    pub fn get_client(&self) -> &Client {
        &self.client
    }

    pub fn get_inventory_action(&self) -> &ClientInventoryAction {
        &self.inventory_action
    }
}

pub fn on_inventory_action(
    events: Res<NetworkEventListener<InventoryActionEvent>>,
    clients: Res<SharedClientsContainer>,
    inventory_manager: Res<SharedInventoryManager>,
    worlds_manager: Res<SharedWorldsManager>,
) {
    let _s = crate::span!("events.on_inventory_action");
    for event in events.0.iter_events() {
        let inventory_action = InventoryAction::from_client_action(
            event.get_client().get_client_id(),
            event.get_inventory_action().clone(),
        );
        let mut inventory_manager = inventory_manager.write();
        inventory_manager.apply_action(event.get_client(), inventory_action, &clients, &worlds_manager);
    }
}
