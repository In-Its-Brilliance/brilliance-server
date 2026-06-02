use bevy_ecs::message::Message;
use bevy_ecs::system::{Commands, Res};
use common::{
    inventory::{inventory::InventoryType, item::Item},
    INVENTORY_BASE, HOTBAR_SLOTS, SPECIAL_INVENTORY_ARTIFACT_SLOT, SPECIAL_INVENTORY_BELT_SLOT,
    SPECIAL_INVENTORY_BOOTS_SLOT, SPECIAL_INVENTORY_BRACER_SLOT, SPECIAL_INVENTORY_CHEST_SLOT,
    SPECIAL_INVENTORY_GLOVES_SLOT, SPECIAL_INVENTORY_HEAD_SLOT, SPECIAL_INVENTORY_NECK_SLOT,
    SPECIAL_INVENTORY_OFFHAND_SLOT, SPECIAL_INVENTORY_PANTS_SLOT, SPECIAL_INVENTORY_RING_0_SLOT,
    SPECIAL_INVENTORY_RING_1_SLOT,
};
use common::utils::events::EventReader;
use network::messages::InventoryAction as ClientInventoryAction;

use crate::{
    clients::{client::Client, clients_container::SharedClientsContainer},
    inventory::SharedInventoryManager,
    items_manager::items_manager::SharedItemsManager,
    network::server::NetworkEventListener,
    worlds::worlds_manager::SharedWorldsManager,
};
use crate::items_manager::item_info::ItemType;
use common::inventory::item::BodyPart;

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
        match action {
            ClientInventoryAction::Move {
                from_inventory,
                from_slot,
                to_inventory,
                to_slot,
                amount,
            } => Self::Move {
                from_inventory: match from_inventory {
                    InventoryType::PlayerPersonal => InventoryTarget::Client(client_id),
                    InventoryType::OtherPlayer(other_client_id) => InventoryTarget::Client(other_client_id),
                    InventoryType::WorldInventory(inventory_id) => InventoryTarget::World(inventory_id),
                },
                from_slot,
                to_inventory: match to_inventory {
                    InventoryType::PlayerPersonal => InventoryTarget::Client(client_id),
                    InventoryType::OtherPlayer(other_client_id) => InventoryTarget::Client(other_client_id),
                    InventoryType::WorldInventory(inventory_id) => InventoryTarget::World(inventory_id),
                },
                to_slot,
                amount,
            },
            ClientInventoryAction::Drop {
                inventory,
                slot,
                amount,
            } => Self::Drop {
                inventory: match inventory {
                    InventoryType::PlayerPersonal => InventoryTarget::Client(client_id),
                    InventoryType::OtherPlayer(other_client_id) => InventoryTarget::Client(other_client_id),
                    InventoryType::WorldInventory(inventory_id) => InventoryTarget::World(inventory_id),
                },
                slot,
                amount,
            },
            ClientInventoryAction::QuickAction {
                from_inventory,
                ..
            } => Self::Close {
                inventory: match from_inventory {
                    InventoryType::PlayerPersonal => InventoryTarget::Client(client_id),
                    InventoryType::OtherPlayer(other_client_id) => InventoryTarget::Client(other_client_id),
                    InventoryType::WorldInventory(inventory_id) => InventoryTarget::World(inventory_id),
                },
            },
            ClientInventoryAction::Close { inventory } => Self::Close {
                inventory: match inventory {
                    InventoryType::PlayerPersonal => InventoryTarget::Client(client_id),
                    InventoryType::OtherPlayer(other_client_id) => InventoryTarget::Client(other_client_id),
                    InventoryType::WorldInventory(inventory_id) => InventoryTarget::World(inventory_id),
                },
            },
        }
    }
}

fn resolve_quick_action(
    client: &Client,
    client_id: u64,
    from_inventory: InventoryType,
    from_slot: u16,
    inventory_manager: &SharedInventoryManager,
    items_manager: &SharedItemsManager,
    clients: &SharedClientsContainer,
    worlds_manager: &SharedWorldsManager,
) -> InventoryAction {
    let from_inventory = match from_inventory {
        InventoryType::PlayerPersonal => InventoryTarget::Client(client_id),
        InventoryType::OtherPlayer(other_client_id) => InventoryTarget::Client(other_client_id),
        InventoryType::WorldInventory(inventory_id) => InventoryTarget::World(inventory_id),
    };

    let inventory_manager = inventory_manager.read();

    let Some(item) = get_inventory_item(
        client,
        &inventory_manager,
        clients,
        worlds_manager,
        &from_inventory,
        from_slot,
    ) else {
        return InventoryAction::Close {
            inventory: from_inventory,
        };
    };

    let Some(to_slot) =
        find_quick_action_target_slot(
            client,
            &inventory_manager,
            clients,
            worlds_manager,
            items_manager,
            &from_inventory,
            from_slot,
            &item,
        )
    else {
        return InventoryAction::Close {
            inventory: from_inventory,
        };
    };

    InventoryAction::Move {
        from_inventory: from_inventory.clone(),
        from_slot,
        to_inventory: from_inventory,
        to_slot,
        amount: item.get_amount(),
    }
}

fn get_inventory_item(
    client: &Client,
    inventory_manager: &crate::inventory::inventory_manager::InventoryManager,
    clients: &SharedClientsContainer,
    worlds_manager: &SharedWorldsManager,
    inventory: &InventoryTarget,
    slot: u16,
) -> Option<Item> {
    match inventory {
        InventoryTarget::Client(target_client_id) if *target_client_id == client.get_client_id() => client
            .with_player_data_mut(|player_data| player_data.get_inventory().get_slot(slot as usize).cloned())
            .flatten(),
        InventoryTarget::Client(client_id) => {
            let clients = clients.read();
            let client = clients.get(client_id)?;
            client
                .with_player_data_mut(|player_data| player_data.get_inventory().get_slot(slot as usize).cloned())
                .flatten()
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
            block_inventory.get_inventory_mut().get_slot(slot as usize).cloned()
        }
    }
}

fn find_quick_action_target_slot(
    client: &Client,
    inventory_manager: &crate::inventory::inventory_manager::InventoryManager,
    clients: &SharedClientsContainer,
    worlds_manager: &SharedWorldsManager,
    items_manager: &SharedItemsManager,
    from_inventory: &InventoryTarget,
    from_slot: u16,
    item: &Item,
) -> Option<u16> {
    const QUICK_ACTION_ARMOR_SLOTS: [usize; 12] = [
        SPECIAL_INVENTORY_HEAD_SLOT,
        SPECIAL_INVENTORY_CHEST_SLOT,
        SPECIAL_INVENTORY_PANTS_SLOT,
        SPECIAL_INVENTORY_BOOTS_SLOT,
        SPECIAL_INVENTORY_NECK_SLOT,
        SPECIAL_INVENTORY_BRACER_SLOT,
        SPECIAL_INVENTORY_GLOVES_SLOT,
        SPECIAL_INVENTORY_OFFHAND_SLOT,
        SPECIAL_INVENTORY_BELT_SLOT,
        SPECIAL_INVENTORY_ARTIFACT_SLOT,
        SPECIAL_INVENTORY_RING_0_SLOT,
        SPECIAL_INVENTORY_RING_1_SLOT,
    ];
    const QUICK_ACTION_GENERAL_SLOT_END: usize = INVENTORY_BASE + HOTBAR_SLOTS;

    if quick_action_target_slot_requires_armor(from_slot as usize) {
        for slot in 0..QUICK_ACTION_GENERAL_SLOT_END {
            if get_inventory_item(client, inventory_manager, clients, worlds_manager, from_inventory, slot as u16).is_none() {
                return Some(slot as u16);
            }
        }
        return None;
    }

    for slot in QUICK_ACTION_ARMOR_SLOTS {
        if slot == from_slot as usize {
            continue;
        }
        if !quick_action_item_fits_slot(items_manager, item, slot) {
            continue;
        }
        if get_inventory_item(client, inventory_manager, clients, worlds_manager, from_inventory, slot as u16).is_none() {
            return Some(slot as u16);
        }
    }

    None
}

fn quick_action_target_slot_requires_armor(slot: usize) -> bool {
    matches!(
        slot,
        SPECIAL_INVENTORY_HEAD_SLOT
            | SPECIAL_INVENTORY_CHEST_SLOT
            | SPECIAL_INVENTORY_PANTS_SLOT
            | SPECIAL_INVENTORY_BOOTS_SLOT
            | SPECIAL_INVENTORY_NECK_SLOT
            | SPECIAL_INVENTORY_BRACER_SLOT
            | SPECIAL_INVENTORY_GLOVES_SLOT
            | SPECIAL_INVENTORY_OFFHAND_SLOT
            | SPECIAL_INVENTORY_BELT_SLOT
            | SPECIAL_INVENTORY_ARTIFACT_SLOT
            | SPECIAL_INVENTORY_RING_0_SLOT
            | SPECIAL_INVENTORY_RING_1_SLOT
    )
}

fn quick_action_item_fits_slot(items_manager: &SharedItemsManager, item: &Item, slot: usize) -> bool {
    let items_manager = items_manager.read();
    let Some(item_type) = items_manager.get_item_type(item) else {
        return false;
    };

    match (slot, item_type) {
        (
            SPECIAL_INVENTORY_HEAD_SLOT,
            ItemType::Armor {
                body_part: BodyPart::Head,
                ..
            },
        ) => true,
        (
            SPECIAL_INVENTORY_CHEST_SLOT,
            ItemType::Armor {
                body_part: BodyPart::Chest,
                ..
            },
        ) => true,
        (
            SPECIAL_INVENTORY_PANTS_SLOT,
            ItemType::Armor {
                body_part: BodyPart::Pants,
                ..
            },
        ) => true,
        (
            SPECIAL_INVENTORY_BOOTS_SLOT,
            ItemType::Armor {
                body_part: BodyPart::Boots,
                ..
            },
        ) => true,
        (SPECIAL_INVENTORY_NECK_SLOT, ItemType::Neck) => true,
        (SPECIAL_INVENTORY_BRACER_SLOT, ItemType::Bracer) => true,
        (SPECIAL_INVENTORY_GLOVES_SLOT, ItemType::Gloves) => true,
        (SPECIAL_INVENTORY_OFFHAND_SLOT, ItemType::Offhand) => true,
        (SPECIAL_INVENTORY_BELT_SLOT, ItemType::Belt) => true,
        (SPECIAL_INVENTORY_ARTIFACT_SLOT, ItemType::Artifact) => true,
        (SPECIAL_INVENTORY_RING_0_SLOT, ItemType::Ring) => true,
        (SPECIAL_INVENTORY_RING_1_SLOT, ItemType::Ring) => true,
        _ => false,
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
    items_manager: Res<SharedItemsManager>,
    worlds_manager: Res<SharedWorldsManager>,
    mut commands: Commands,
) {
    let _s = crate::span!("events.on_inventory_action");
    for event in events.0.iter_events() {
        let inventory_action = match event.get_inventory_action().clone() {
            ClientInventoryAction::QuickAction {
                from_inventory,
                from_slot,
            } => resolve_quick_action(
                event.get_client(),
                event.get_client().get_client_id(),
                from_inventory,
                from_slot,
                &inventory_manager,
                &items_manager,
                &clients,
                &worlds_manager,
            ),
            other => InventoryAction::from_client_action(event.get_client().get_client_id(), other),
        };
        let mut inventory_manager = inventory_manager.write();
        if let Err(e) = inventory_manager.apply_action(
            event.get_client(),
            inventory_action,
            &clients,
            &items_manager,
            &worlds_manager,
            &mut commands,
        ) {
            if let Some(msg) = e {
                event
                    .get_client()
                    .send_console_message(format!("&4Inventory action rejected: &c{}", msg));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        clients::client::ClientInfo,
        items_manager::{
            item_info::{ItemDisplay, ItemInfo},
            items_manager::ItemsManager,
        },
        plugins::plugins_manager::PluginsManager,
        utils::Shared,
    };
    use common::{
        inventory::item::Item, server_storage::taits::IServerStorage, timed_lock,
        utils::srotage_settings::StorageSettings, ServerStorageManager,
    };
    use std::sync::Arc;

    fn make_client() -> Client {
        let client = crate::clients::client::Client::test();
        client.set_client_info(ClientInfo::new(
            &crate::network::events::on_connection_info::PlayerConnectionInfoEvent::new(
                client.clone(),
                "test_player".to_string(),
                "test".to_string(),
                "test".to_string(),
                "test".to_string(),
            ),
        ));

        let storage = ServerStorageManager::init(StorageSettings::in_memory()).expect("in-memory storage must init");
        client
            .read_player_data(&storage)
            .expect("player data must load from storage");
        client
    }

    fn make_items_manager() -> SharedItemsManager {
        let items_manager = Shared::new(Arc::new(timed_lock!(ItemsManager::default(), "test_items_manager")));
        items_manager
            .write()
            .add_item(
                &PluginsManager::default(),
                ItemInfo::create(
                    "test_helmet",
                    ItemType::Armor {
                        body_part: BodyPart::Head,
                        model: "default://assets/models/generic/replace.glb".to_string(),
                    },
                    ItemDisplay::Icon("default://assets/resources/default/icons_helmet/icon1.png".to_string()),
                    "Test Helmet",
                    "Test Helmet",
                    1,
                ),
            )
            .expect("test helmet must be registered");
        items_manager
    }

    fn make_clients() -> SharedClientsContainer {
        SharedClientsContainer::new(Arc::new(timed_lock!(
            crate::clients::clients_container::ClientsContainer::default(),
            "test_clients"
        )))
    }

    #[test]
    fn quick_action_resolves_into_equipping_helmet() {
        let client = make_client();
        let items_manager = make_items_manager();
        let clients = make_clients();
        let inventory_manager = SharedInventoryManager::new(Arc::new(timed_lock!(
            crate::inventory::inventory_manager::InventoryManager::default(),
            "inventory_manager"
        )));
        let worlds_manager = SharedWorldsManager::new(Arc::new(timed_lock!(
            crate::worlds::worlds_manager::WorldsManager::default(),
            "test_worlds"
        )));

        client.with_player_data_mut(|player_data| {
            player_data.get_inventory_mut().set_slot(0, Item::create("test_helmet"));
        });

        let action = resolve_quick_action(
            &client,
            client.get_client_id(),
            InventoryType::PlayerPersonal,
            0,
            &inventory_manager,
            &items_manager,
            &clients,
            &worlds_manager,
        );

        match action {
            InventoryAction::Move {
                from_slot, to_slot, ..
            } => {
                assert_eq!(from_slot, 0);
                assert_eq!(to_slot, SPECIAL_INVENTORY_HEAD_SLOT as u16);
            }
            other => panic!("expected move, got {:?}", other),
        }
    }

    #[test]
    fn quick_action_resolves_into_unequipping_helmet() {
        let client = make_client();
        let items_manager = make_items_manager();
        let clients = make_clients();
        let inventory_manager = SharedInventoryManager::new(Arc::new(timed_lock!(
            crate::inventory::inventory_manager::InventoryManager::default(),
            "inventory_manager"
        )));
        let worlds_manager = SharedWorldsManager::new(Arc::new(timed_lock!(
            crate::worlds::worlds_manager::WorldsManager::default(),
            "test_worlds"
        )));

        client.with_player_data_mut(|player_data| {
            player_data
                .get_inventory_mut()
                .set_slot(SPECIAL_INVENTORY_HEAD_SLOT, Item::create("test_helmet"));
        });

        let action = resolve_quick_action(
            &client,
            client.get_client_id(),
            InventoryType::PlayerPersonal,
            SPECIAL_INVENTORY_HEAD_SLOT as u16,
            &inventory_manager,
            &items_manager,
            &clients,
            &worlds_manager,
        );

        match action {
            InventoryAction::Move {
                from_slot, to_slot, ..
            } => {
                assert_eq!(from_slot, SPECIAL_INVENTORY_HEAD_SLOT as u16);
                assert_eq!(to_slot, 0);
            }
            other => panic!("expected move, got {:?}", other),
        }
    }
}
