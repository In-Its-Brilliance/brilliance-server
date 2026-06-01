mod close;
mod drop;
mod helpers;
mod hooks;
mod move_ops;

use crate::items_manager::item_info::ItemType;
use bevy_ecs::system::Commands;
#[cfg(test)]
use bevy_ecs::world::{CommandQueue, World};
use crate::{
    clients::client::Client,
    clients::clients_container::SharedClientsContainer,
    inventory::inventory_manager::InventoryManager,
    items_manager::items_manager::SharedItemsManager,
    network::events::on_inventory_action::{InventoryAction, InventoryTarget},
    worlds::worlds_manager::SharedWorldsManager,
};

use helpers::InventoryActionCtx;
use hooks::{inventory_slot_allowed, item_fits_slot, target_slot_requires_armor};

pub struct InventoryActions;

impl InventoryActions {
    pub fn apply_action(
        client: &Client,
        action: InventoryAction,
        clients: &SharedClientsContainer,
        items_manager: &SharedItemsManager,
        inventory_manager: &mut InventoryManager,
        worlds_manager: &SharedWorldsManager,
        commands: &mut Commands,
    ) -> Result<(), Option<String>> {
        let ctx = InventoryActionCtx {
            client,
            clients,
            items_manager,
            worlds_manager,
        };

        Self::authorize_action(&ctx, inventory_manager, &action)?;
        hooks::before_action(&ctx, inventory_manager, &action)?;

        match action {
            InventoryAction::Move {
                from_inventory,
                from_slot,
                to_inventory,
                to_slot,
                amount,
            } => move_ops::apply_move(
                &ctx,
                commands,
                inventory_manager,
                from_inventory,
                from_slot,
                to_inventory,
                to_slot,
                amount,
            ),
            InventoryAction::Drop {
                inventory,
                slot,
                amount,
            } => drop::apply_drop(&ctx, commands, inventory_manager, inventory, slot, amount),
            InventoryAction::Close { inventory } => close::apply_close(&ctx, inventory_manager, inventory),
        }
        Ok(())
    }

    fn authorize_action(
        ctx: &InventoryActionCtx<'_>,
        inventory_manager: &InventoryManager,
        action: &InventoryAction,
    ) -> Result<(), String> {
        match action {
            InventoryAction::Move {
                from_inventory,
                to_inventory,
                ..
            } => {
                Self::authorize_inventory_target(ctx, inventory_manager, from_inventory)?;
                Self::authorize_inventory_target(ctx, inventory_manager, to_inventory)?;
            }
            InventoryAction::Drop { inventory, .. } | InventoryAction::Close { inventory } => {
                Self::authorize_inventory_target(ctx, inventory_manager, inventory)?;
            }
        }

        Ok(())
    }

    fn authorize_inventory_target(
        ctx: &InventoryActionCtx<'_>,
        inventory_manager: &InventoryManager,
        inventory_target: &InventoryTarget,
    ) -> Result<(), String> {
        let client_id = ctx.client.get_client_id();
        let world_entity = ctx
            .client
            .get_world_entity()
            .map(|world_entity| world_entity.get_entity());
        authorize_inventory_target(client_id, world_entity, inventory_manager, inventory_target)
    }

}

fn authorize_inventory_target(
    client_id: u64,
    world_entity: Option<bevy::prelude::Entity>,
    inventory_manager: &InventoryManager,
    inventory_target: &InventoryTarget,
) -> Result<(), String> {
    match inventory_target {
        InventoryTarget::Client(target_client_id) if *target_client_id == client_id => Ok(()),
        InventoryTarget::Client(target_client_id) => {
            Err(format!("You cannot act on player {} inventory", target_client_id))
        }
        InventoryTarget::World(inventory_id) => {
            let Some(world_entity) = world_entity else {
                return Err(format!("You cannot act on world inventory {}", inventory_id));
            };

            let Some(watchers) = inventory_manager.state().get_inventory_watchers(inventory_id) else {
                return Err(format!("World inventory {} is not available", inventory_id));
            };

            if !watchers.iter().any(|watcher| *watcher == world_entity) {
                return Err(format!("World inventory {} is not open for you", inventory_id));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use bevy::prelude::Entity;
    use common::{
        inventory::item::Item,
        server_storage::taits::IServerStorage,
        timed_lock,
        utils::srotage_settings::StorageSettings,
        INVENTORY_SLOTS, SPECIAL_INVENTORY_HEAD_SLOT, SPECIAL_INVENTORY_NECK_SLOT, SPECIAL_INVENTORY_RING_0_SLOT,
    };

    use super::*;
    use crate::{
        clients::{client::ClientInfo, clients_container::SharedClientsContainer},
        items_manager::item_info::{ItemDisplay, ItemInfo},
        items_manager::items_manager::ItemsManager,
        network::events::on_inventory_action::InventoryAction,
        plugins::plugins_manager::PluginsManager,
        utils::Shared,
        worlds::worlds_manager::SharedWorldsManager,
    };
    use common::ServerStorageManager;

    #[test]
    fn allows_own_client_inventory() {
        let inventory_manager = InventoryManager::default();
        let target = InventoryTarget::Client(7);

        authorize_inventory_target(7, None, &inventory_manager, &target).expect("own inventory access must be allowed");
    }

    #[test]
    fn rejects_other_client_inventory() {
        let inventory_manager = InventoryManager::default();
        let target = InventoryTarget::Client(9);

        let err = authorize_inventory_target(7, None, &inventory_manager, &target)
            .expect_err("foreign inventory access must be rejected");

        assert_eq!(err, "You cannot act on player 9 inventory");
    }

    #[test]
    fn rejects_move_into_unwatched_world_inventory() {
        let client_id = 7;
        let world_entity = Some(Entity::from_raw_u32(42).unwrap());
        let inventory_manager = InventoryManager::default();
        let target = InventoryTarget::World(100);

        let err = authorize_inventory_target(client_id, world_entity, &inventory_manager, &target)
            .expect_err("move into unwatched inventory must be rejected");

        assert_eq!(err, "World inventory 100 is not available");
    }

    #[test]
    fn allows_last_inventory_slot() {
        assert!(inventory_slot_allowed((INVENTORY_SLOTS - 1) as u16));
    }

    #[test]
    fn rejects_inventory_slot_past_the_end() {
        assert!(!inventory_slot_allowed(INVENTORY_SLOTS as u16));
    }

    #[test]
    fn rejects_non_armor_item_for_head_slot() {
        let items_manager = Shared::new(Arc::new(timed_lock!(ItemsManager::default(), "test_items_manager")));
        items_manager
            .write()
            .add_item(
                &PluginsManager::default(),
                ItemInfo::create(
                    "test_other",
                    ItemType::other(),
                    ItemDisplay::Icon("default://assets/resources/default/icons_artefacts/icon1.png".to_string()),
                    "Test Other",
                    "Test Other",
                    1,
                ),
            )
            .expect("test item must be registered");

        let item = Item::create("test_other");

        assert!(target_slot_requires_armor(SPECIAL_INVENTORY_HEAD_SLOT));
        assert!(!item_fits_slot(&items_manager, &item, SPECIAL_INVENTORY_HEAD_SLOT));
    }

    #[test]
    fn allows_neck_item_for_neck_slot() {
        let items_manager = Shared::new(Arc::new(timed_lock!(ItemsManager::default(), "test_items_manager")));
        items_manager
            .write()
            .add_item(
                &PluginsManager::default(),
                ItemInfo::create(
                    "test_neck",
                    ItemType::Neck,
                    ItemDisplay::Icon("default://assets/resources/default/icons_artefacts/icon1.png".to_string()),
                    "Test Neck",
                    "Test Neck",
                    1,
                ),
            )
            .expect("test item must be registered");

        let item = Item::create("test_neck");

        assert!(target_slot_requires_armor(SPECIAL_INVENTORY_NECK_SLOT));
        assert!(item_fits_slot(&items_manager, &item, SPECIAL_INVENTORY_NECK_SLOT));
    }

    #[test]
    fn allows_ring_item_for_first_ring_slot() {
        let items_manager = Shared::new(Arc::new(timed_lock!(ItemsManager::default(), "test_items_manager")));
        items_manager
            .write()
            .add_item(
                &PluginsManager::default(),
                ItemInfo::create(
                    "test_ring",
                    ItemType::Ring,
                    ItemDisplay::Icon("default://assets/resources/default/icons_artefacts/icon1.png".to_string()),
                    "Test Ring",
                    "Test Ring",
                    1,
                ),
            )
            .expect("test item must be registered");

        let item = Item::create("test_ring");

        assert!(target_slot_requires_armor(SPECIAL_INVENTORY_RING_0_SLOT));
        assert!(item_fits_slot(&items_manager, &item, SPECIAL_INVENTORY_RING_0_SLOT));
    }

    #[test]
    fn rejects_moving_non_armor_item_into_head_slot() {
        let client = crate::clients::client::Client::test();
        client.set_client_info(ClientInfo::new(&crate::network::events::on_connection_info::PlayerConnectionInfoEvent::new(
            client.clone(),
            "test_player".to_string(),
            "test".to_string(),
            "test".to_string(),
            "test".to_string(),
        )));

        let storage = ServerStorageManager::init(StorageSettings::in_memory()).expect("in-memory storage must init");
        client
            .read_player_data(&storage)
            .expect("player data must load from storage");

        let items_manager = Shared::new(Arc::new(timed_lock!(ItemsManager::default(), "test_items_manager")));
        items_manager
            .write()
            .add_item(
                &PluginsManager::default(),
                ItemInfo::create(
                    "test_other",
                    ItemType::other(),
                    ItemDisplay::Icon("default://assets/resources/default/icons_artefacts/icon1.png".to_string()),
                    "Test Other",
                    "Test Other",
                    1,
                ),
            )
            .expect("test item must be registered");

        client.with_player_data_mut(|player_data| {
            player_data.get_inventory_mut().set_slot(0, Item::create("test_other"));
        });

        let clients = SharedClientsContainer::new(Arc::new(timed_lock!(
            crate::clients::clients_container::ClientsContainer::default(),
            "test_clients"
        )));
        let worlds_manager = SharedWorldsManager::new(Arc::new(timed_lock!(
            crate::worlds::worlds_manager::WorldsManager::default(),
            "test_worlds"
        )));
        let mut inventory_manager = InventoryManager::default();
        let world = World::new();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);

        let result = InventoryActions::apply_action(
            &client,
            InventoryAction::Move {
                from_inventory: InventoryTarget::Client(client.get_client_id()),
                from_slot: 0,
                to_inventory: InventoryTarget::Client(client.get_client_id()),
                to_slot: SPECIAL_INVENTORY_HEAD_SLOT as u16,
                amount: 1,
            },
            &clients,
            &items_manager,
            &mut inventory_manager,
            &worlds_manager,
            &mut commands,
        );

        assert!(result.is_err(), "move into head slot with non-armor must be rejected");
        let error = result.expect_err("move into head slot with non-armor must fail");
        assert!(error.is_none(), "there is must be no error, its just silent denial");
        client.with_player_data_mut(|player_data| {
            assert!(player_data.get_inventory().get_slot(SPECIAL_INVENTORY_HEAD_SLOT).is_none());
            assert!(player_data.get_inventory().get_slot(0).is_some());
        });
    }
}
