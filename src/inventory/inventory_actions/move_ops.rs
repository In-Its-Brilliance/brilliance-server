use common::inventory::{inventory::Inventory, item::Item};
use network::messages::InventorySlotChange;

use crate::{
    inventory::inventory_manager::InventoryManager, items_manager::items_manager::SharedItemsManager,
    network::events::on_inventory_action::InventoryTarget,
};

use super::helpers::{
    broadcast_inventory_changes, calculate_transfer_amount, with_inventory_mut, with_inventory_ref, InventoryActionCtx,
};

pub(crate) fn apply_move(
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &mut InventoryManager,
    from_inventory: InventoryTarget,
    from_slot: u16,
    to_inventory: InventoryTarget,
    to_slot: u16,
    amount: u16,
) {
    if from_inventory == to_inventory {
        let changes = with_inventory_mut(ctx, inventory_manager, &from_inventory, |inventory| {
            move_within_inventory(
                inventory,
                ctx.items_manager,
                from_slot as usize,
                to_slot as usize,
                amount,
            )
        });
        broadcast_inventory_changes(ctx, inventory_manager, from_inventory, changes);
        return;
    }

    let Some((from_type, from_changes, to_type, to_changes)) = move_between_inventories(
        ctx,
        inventory_manager,
        from_inventory.clone(),
        from_slot as usize,
        to_inventory.clone(),
        to_slot as usize,
        amount,
    ) else {
        return;
    };

    broadcast_inventory_changes(ctx, inventory_manager, from_type, Some(from_changes));
    broadcast_inventory_changes(ctx, inventory_manager, to_type, Some(to_changes));
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
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &InventoryManager,
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
    let Some(source_item) = with_inventory_ref(ctx, inventory_manager, &from_inventory, |inventory| {
        inventory.get_slot(from_slot).cloned()
    })
    .flatten() else {
        return None;
    };
    let target_item = with_inventory_ref(ctx, inventory_manager, &to_inventory, |inventory| {
        inventory.get_slot(to_slot).cloned()
    })
    .flatten();

    let requested = amount;

    if let Some(existing) = target_item.as_ref() {
        if !existing.can_stack_with(&source_item) {
            if requested != source_item.get_amount() {
                return None;
            }

            return swap_between_inventories(ctx, inventory_manager, from_inventory, from_slot, to_inventory, to_slot);
        }
    }

    let accepted = calculate_transfer_amount(ctx.items_manager, &source_item, target_item.as_ref(), requested);
    if accepted == 0 {
        return None;
    }

    let Some((from_type, moved_item, from_changes)) =
        extract_item(ctx, inventory_manager, &from_inventory, from_slot, accepted)
    else {
        return None;
    };

    let Some((to_type, to_changes)) = insert_item(ctx, inventory_manager, &to_inventory, to_slot, moved_item.clone())
    else {
        let restore_changes = with_inventory_mut(ctx, inventory_manager, &from_type, |inventory| {
            inventory.set_slot_option(from_slot, Some(source_item.clone()));
            vec![InventorySlotChange {
                slot: from_slot,
                item: Some(ctx.items_manager.read().to_client_item(&source_item)),
            }]
        });
        broadcast_inventory_changes(ctx, inventory_manager, from_type, restore_changes);
        return None;
    };

    Some((from_type, from_changes, to_type, to_changes))
}

fn swap_between_inventories(
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &InventoryManager,
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
    let Some((from_type, from_item, _)) = take_slot(ctx, inventory_manager, &from_inventory, from_slot) else {
        return None;
    };
    let Some((to_type, to_item, _)) = take_slot(ctx, inventory_manager, &to_inventory, to_slot) else {
        let _ = with_inventory_mut(ctx, inventory_manager, &from_type, |inventory| {
            inventory.set_slot_option(from_slot, Some(from_item.clone()));
        });
        return None;
    };

    let _ = with_inventory_mut(ctx, inventory_manager, &from_type, |inventory| {
        inventory.set_slot_option(from_slot, Some(to_item.clone()));
    });
    let _ = with_inventory_mut(ctx, inventory_manager, &to_type, |inventory| {
        inventory.set_slot_option(to_slot, Some(from_item.clone()));
    });

    Some((
        from_type,
        vec![InventorySlotChange {
            slot: from_slot,
            item: Some(ctx.items_manager.read().to_client_item(&to_item)),
        }],
        to_type,
        vec![InventorySlotChange {
            slot: to_slot,
            item: Some(ctx.items_manager.read().to_client_item(&from_item)),
        }],
    ))
}

fn take_slot(
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &InventoryManager,
    inventory_target: &InventoryTarget,
    slot: usize,
) -> Option<(InventoryTarget, Item, Vec<InventorySlotChange>)> {
    let mut item = None;
    let changes = with_inventory_mut(ctx, inventory_manager, inventory_target, |inventory| {
        item = inventory.take_slot(slot);
        item.as_ref()
            .map(|_| vec![InventorySlotChange { slot, item: None }])
            .unwrap_or_default()
    })?;
    Some((inventory_target.clone(), item?, changes))
}

fn extract_item(
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &InventoryManager,
    inventory_target: &InventoryTarget,
    slot: usize,
    amount: u16,
) -> Option<(InventoryTarget, Item, Vec<InventorySlotChange>)> {
    let mut extracted = None;
    let mut source_change = Vec::new();
    with_inventory_mut(ctx, inventory_manager, inventory_target, |inventory| {
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
                    .map(|item| ctx.items_manager.read().to_client_item(&item)),
            }];
        }
        extracted = Some(moved);
    })?;
    Some((inventory_target.clone(), extracted?, source_change))
}

fn insert_item(
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &InventoryManager,
    inventory_target: &InventoryTarget,
    slot: usize,
    item: Item,
) -> Option<(InventoryTarget, Vec<InventorySlotChange>)> {
    let changes = with_inventory_mut(ctx, inventory_manager, inventory_target, |inventory| {
        match inventory.get_slot(slot).cloned() {
            Some(existing) if existing.can_stack_with(&item) => {
                let max_stack_size = ctx.items_manager.read().get_max_stack_size(&item);
                let space = max_stack_size.saturating_sub(existing.get_amount());
                if space == 0 {
                    return Vec::new();
                }
                let added = item.get_amount().min(space);
                let updated = existing.clone().amount(existing.get_amount() + added);
                inventory.set_slot_option(slot, Some(updated.clone()));
                vec![InventorySlotChange {
                    slot,
                    item: Some(ctx.items_manager.read().to_client_item(&updated)),
                }]
            }
            None => {
                inventory.set_slot_option(slot, Some(item.clone()));
                vec![InventorySlotChange {
                    slot,
                    item: Some(ctx.items_manager.read().to_client_item(&item)),
                }]
            }
            _ => Vec::new(),
        }
    })?;
    if changes.is_empty() {
        return None;
    }
    Some((inventory_target.clone(), changes))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use common::utils::debug::SmartRwLock;

    use super::*;
    use crate::{items_manager::items_manager::ItemsManager, utils::Shared};

    fn shared_items_manager() -> SharedItemsManager {
        Shared::new(Arc::new(SmartRwLock::new(
            ItemsManager::default(),
            "inventory_actions_move_ops_test_items_manager",
        )))
    }

    #[test]
    fn moves_partial_stack_into_empty_slot() {
        let items_manager = shared_items_manager();
        let mut inventory = Inventory::create(4);
        inventory.set_slot(0, Item::create("apple").amount(10));

        let changes = move_within_inventory(&mut inventory, &items_manager, 0, 1, 4);

        assert_eq!(inventory.get_slot(0).unwrap().get_amount(), 6);
        assert_eq!(inventory.get_slot(1).unwrap().get_amount(), 4);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].slot, 0);
        assert_eq!(changes[1].slot, 1);
    }

    #[test]
    fn merges_stackable_items() {
        let items_manager = shared_items_manager();
        let mut inventory = Inventory::create(4);
        inventory.set_slot(0, Item::create(1u16).amount(5));
        inventory.set_slot(1, Item::create(1u16).amount(3));

        let changes = move_within_inventory(&mut inventory, &items_manager, 0, 1, 2);

        assert_eq!(inventory.get_slot(0).unwrap().get_amount(), 3);
        assert_eq!(inventory.get_slot(1).unwrap().get_amount(), 5);
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn swaps_non_stackable_items_only_for_full_stack_move() {
        let items_manager = shared_items_manager();
        let mut inventory = Inventory::create(4);
        inventory.set_slot(0, Item::create("apple").amount(2));
        inventory.set_slot(1, Item::create("stone").amount(1));

        let changes = move_within_inventory(&mut inventory, &items_manager, 0, 1, 2);

        assert_eq!(
            inventory.get_slot(0).unwrap().get_item_kind(),
            Item::create("stone").get_item_kind()
        );
        assert_eq!(
            inventory.get_slot(1).unwrap().get_item_kind(),
            Item::create("apple").get_item_kind()
        );
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn rejects_partial_swap_for_non_stackable_items() {
        let items_manager = shared_items_manager();
        let mut inventory = Inventory::create(4);
        inventory.set_slot(0, Item::create("apple").amount(2));
        inventory.set_slot(1, Item::create("stone").amount(1));

        let changes = move_within_inventory(&mut inventory, &items_manager, 0, 1, 1);

        assert!(changes.is_empty());
        assert_eq!(inventory.get_slot(0).unwrap().get_amount(), 2);
        assert_eq!(inventory.get_slot(1).unwrap().get_amount(), 1);
    }

    #[test]
    fn ignores_move_into_same_slot() {
        let items_manager = shared_items_manager();
        let mut inventory = Inventory::create(4);
        inventory.set_slot(0, Item::create("apple").amount(2));

        let changes = move_within_inventory(&mut inventory, &items_manager, 0, 0, 1);

        assert!(changes.is_empty());
        assert_eq!(inventory.get_slot(0).unwrap().get_amount(), 2);
    }
}
