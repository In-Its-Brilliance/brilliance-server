mod close;
mod drop;
mod helpers;
mod move_ops;

use crate::{
    clients::client::Client,
    clients::clients_container::SharedClientsContainer,
    inventory::inventory_manager::InventoryManager,
    items_manager::items_manager::SharedItemsManager,
    network::events::on_inventory_action::{InventoryAction, InventoryTarget},
    worlds::worlds_manager::SharedWorldsManager,
};
use common::{
    inventory::{item::BodyPart, item::Item},
    INVENTORY_SLOTS, SPECIAL_INVENTORY_BOOTS_SLOT, SPECIAL_INVENTORY_CHEST_SLOT,
    SPECIAL_INVENTORY_HEAD_SLOT, SPECIAL_INVENTORY_PANTS_SLOT,
};
use crate::items_manager::item_info::ItemType;

use helpers::{with_inventory_ref, InventoryActionCtx};

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
        let ctx = InventoryActionCtx {
            client,
            clients,
            items_manager,
            worlds_manager,
        };

        Self::authorize_action(&ctx, inventory_manager, &action)?;
        match Self::before_action(&ctx, inventory_manager, &action) {
            Ok(()) => {}
            Err(None) => return Ok(()),
            Err(Some(error)) => return Err(error),
        }

        match action {
            InventoryAction::Move {
                from_inventory,
                from_slot,
                to_inventory,
                to_slot,
                amount,
            } => move_ops::apply_move(
                &ctx,
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
            } => drop::apply_drop(&ctx, inventory_manager, inventory, slot, amount),
            InventoryAction::Close { inventory } => close::apply_close(&ctx, inventory_manager, inventory),
        }
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

    fn before_action(
        ctx: &InventoryActionCtx<'_>,
        inventory_manager: &InventoryManager,
        action: &InventoryAction,
    ) -> Result<(), Option<String>> {
        match action {
            InventoryAction::Move {
                from_inventory,
                from_slot,
                to_inventory: _,
                to_slot,
                amount,
            } => {
                if !inventory_slot_allowed(*from_slot) {
                    return Err(Some(format!("source slot is not allowed: {}", from_slot)));
                }
                if !inventory_slot_allowed(*to_slot) {
                    return Err(Some(format!("target slot is not allowed: {}", to_slot)));
                }
                if *amount == 0 {
                    return Err(Some(format!("empty amount is not allowed: {}", amount)));
                }

                let Some(source_item) = with_inventory_ref(ctx, inventory_manager, from_inventory, |inventory| {
                    inventory.get_slot(*from_slot as usize).cloned()
                })
                .flatten() else {
                    return Err(Some(format!("source slot is empty: {}", from_slot)));
                };

                if *amount > source_item.get_amount() {
                    return Err(Some(format!(
                        "move amount {} exceeds source amount {}",
                        amount,
                        source_item.get_amount()
                    )));
                }

                if target_slot_requires_armor(*to_slot as usize)
                    && !item_fits_slot(ctx.items_manager, &source_item, *to_slot as usize)
                {
                    return Err(Some(format!("item type does not fit target slot: {}", to_slot)));
                }

                Ok(())
            }
            InventoryAction::Drop { inventory, slot, amount } => {
                if !inventory_slot_allowed(*slot) {
                    return Err(Some(format!("slot is not allowed: {}", slot)));
                }
                if *amount == 0 {
                    return Err(Some(format!("empty amount is not allowed: {}", amount)));
                }

                let Some(item) = with_inventory_ref(ctx, inventory_manager, inventory, |inventory| {
                    inventory.get_slot(*slot as usize).cloned()
                })
                .flatten() else {
                    return Err(Some(format!("slot is empty: {}", slot)));
                };

                if *amount > item.get_amount() {
                    return Err(Some(format!(
                        "drop amount {} exceeds source amount {}",
                        amount,
                        item.get_amount()
                    )));
                }

                if target_slot_requires_armor(*slot as usize)
                    && !item_fits_slot(ctx.items_manager, &item, *slot as usize)
                {
                    return Err(Some(format!("item type does not fit slot: {}", slot)));
                }

                Ok(())
            }
            InventoryAction::Close { .. } => Ok(()),
        }
    }
}

fn inventory_slot_allowed(slot: u16) -> bool {
    (slot as usize) < INVENTORY_SLOTS
}

fn target_slot_requires_armor(slot: usize) -> bool {
    matches!(
        slot,
        SPECIAL_INVENTORY_HEAD_SLOT | SPECIAL_INVENTORY_CHEST_SLOT | SPECIAL_INVENTORY_PANTS_SLOT
            | SPECIAL_INVENTORY_BOOTS_SLOT
    )
}

fn item_fits_slot(items_manager: &SharedItemsManager, item: &Item, slot: usize) -> bool {
    let items_manager = items_manager.read();
    let Some(item_type) = items_manager.get_item_type(item) else {
        return false;
    };

    match (slot, item_type) {
        (SPECIAL_INVENTORY_HEAD_SLOT, ItemType::Armor { body_part: BodyPart::Head, .. }) => true,
        (SPECIAL_INVENTORY_CHEST_SLOT, ItemType::Armor { body_part: BodyPart::Chest, .. }) => true,
        (SPECIAL_INVENTORY_PANTS_SLOT, ItemType::Armor { body_part: BodyPart::Pants, .. }) => true,
        (SPECIAL_INVENTORY_BOOTS_SLOT, ItemType::Armor { body_part: BodyPart::Boots, .. }) => true,
        _ => false,
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
        InventoryTarget::Client(target_client_id) => Err(format!(
            "You cannot act on player {} inventory",
            target_client_id
        )),
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
    use bevy::prelude::Entity;
    use common::INVENTORY_SLOTS;

    use super::*;

    #[test]
    fn allows_own_client_inventory() {
        let inventory_manager = InventoryManager::default();
        let target = InventoryTarget::Client(7);

        authorize_inventory_target(7, None, &inventory_manager, &target)
            .expect("own inventory access must be allowed");
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
}
