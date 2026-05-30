use common::{inventory::inventory::Inventory, INVENTORY_BASE};
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
    validate_inventory_slot(&inventory, slot)?;
    let changes = with_inventory_mut(ctx, inventory_manager, &inventory, |inventory_data| {
        drop_stack(inventory_data, ctx.items_manager, slot as usize, amount)
    });
    broadcast_inventory_changes(ctx, inventory_manager, inventory, changes);
    Ok(())
}

fn validate_inventory_slot(inventory: &InventoryTarget, slot: u16) -> Result<(), String> {
    match inventory {
        InventoryTarget::Client(_) if slot as usize >= INVENTORY_BASE => Err(format!(
            "player inventory slot {} is out of range 0..{}",
            slot,
            INVENTORY_BASE
        )),
        _ => Ok(()),
    }
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
    use super::*;

    #[test]
    fn rejects_player_inventory_slot_out_of_range() {
        let err = validate_inventory_slot(&InventoryTarget::Client(7), INVENTORY_BASE as u16)
            .expect_err("must reject player slot past the end");
        assert_eq!(
            err,
            format!(
                "player inventory slot {} is out of range 0..{}",
                INVENTORY_BASE,
                INVENTORY_BASE
            )
        );
    }
}
