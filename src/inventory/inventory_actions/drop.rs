use common::inventory::inventory::Inventory;
use network::messages::InventorySlotChange;

use crate::{
    inventory::inventory_manager::InventoryManager, items_manager::items_manager::SharedItemsManager,
    network::events::on_inventory_action::InventoryTarget,
};

use super::helpers::{broadcast_inventory_changes, with_inventory_mut, InventoryActionCtx};

pub(crate) fn apply_drop(
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &mut InventoryManager,
    inventory: InventoryTarget,
    slot: u16,
    amount: u16,
) -> Result<(), String> {
    let changes = with_inventory_mut(ctx, inventory_manager, &inventory, |inventory_data| {
        drop_stack(inventory_data, ctx.items_manager, slot as usize, amount)
    });
    broadcast_inventory_changes(ctx, inventory_manager, inventory, changes);
    Ok(())
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use common::{inventory::item::Item, utils::debug::SmartRwLock};

    use super::*;
    use crate::{items_manager::items_manager::ItemsManager, utils::Shared};

    fn shared_items_manager() -> SharedItemsManager {
        Shared::new(Arc::new(SmartRwLock::new(
            ItemsManager::default(),
            "inventory_actions_drop_test_items_manager",
        )))
    }

    #[test]
    fn drops_part_of_stack() {
        let items_manager = shared_items_manager();
        let mut inventory = Inventory::create(4);
        inventory.set_slot(0, Item::create("apple").amount(5));

        let changes = drop_stack(&mut inventory, &items_manager, 0, 2);

        assert_eq!(inventory.get_slot(0).unwrap().get_amount(), 3);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].slot, 0);
        assert_eq!(changes[0].item.as_ref().unwrap().get_amount(), 3);
    }

    #[test]
    fn drops_full_stack() {
        let items_manager = shared_items_manager();
        let mut inventory = Inventory::create(4);
        inventory.set_slot(0, Item::create("apple").amount(5));

        let changes = drop_stack(&mut inventory, &items_manager, 0, 5);

        assert!(inventory.get_slot(0).is_none());
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].slot, 0);
        assert!(changes[0].item.is_none());
    }

    #[test]
    fn ignores_zero_amount_drop() {
        let items_manager = shared_items_manager();
        let mut inventory = Inventory::create(4);
        inventory.set_slot(0, Item::create("apple").amount(5));

        let changes = drop_stack(&mut inventory, &items_manager, 0, 0);

        assert!(changes.is_empty());
        assert_eq!(inventory.get_slot(0).unwrap().get_amount(), 5);
    }

    #[test]
    fn ignores_drop_from_empty_slot() {
        let items_manager = shared_items_manager();
        let mut inventory = Inventory::create(4);

        let changes = drop_stack(&mut inventory, &items_manager, 0, 2);

        assert!(changes.is_empty());
        assert!(inventory.get_slot(0).is_none());
    }
}
