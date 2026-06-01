use crate::items_manager::item_info::ItemType;
use bevy_ecs::system::Commands;
use crate::{
    clients::client::Client,
    entities::{commands::UpdatePlayerComponent, skin::EntitySkinComponent, EntityComponent},
    inventory::inventory_manager::InventoryManager,
    items_manager::items_manager::SharedItemsManager,
    network::events::on_inventory_action::{InventoryAction, InventoryTarget},
};
use common::{
    inventory::{item::BodyPart, item::Item},
    INVENTORY_SLOTS, SPECIAL_INVENTORY_ARTIFACT_SLOT, SPECIAL_INVENTORY_BELT_SLOT, SPECIAL_INVENTORY_BOOTS_SLOT,
    SPECIAL_INVENTORY_BRACER_SLOT, SPECIAL_INVENTORY_CHEST_SLOT, SPECIAL_INVENTORY_GLOVES_SLOT,
    SPECIAL_INVENTORY_HEAD_SLOT, SPECIAL_INVENTORY_NECK_SLOT, SPECIAL_INVENTORY_OFFHAND_SLOT,
    SPECIAL_INVENTORY_PANTS_SLOT, SPECIAL_INVENTORY_RING_0_SLOT, SPECIAL_INVENTORY_RING_1_SLOT,
};
use network::messages::InventorySlotChange;

use super::helpers::{with_inventory_ref, InventoryActionCtx};

pub(crate) fn before_action(
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
                return Err(None);
            }

            Ok(())
        }
        InventoryAction::Drop {
            inventory,
            slot,
            amount,
        } => {
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

pub(crate) fn after_inventory_modified(
    ctx: &InventoryActionCtx<'_>,
    commands: &mut Commands,
    inventory_target: &InventoryTarget,
    changes: &[InventorySlotChange],
) {
    if !armor_slot_changed(changes) {
        return;
    }

    let client = match inventory_target {
        InventoryTarget::Client(client_id) if *client_id == ctx.client.get_client_id() => ctx.client.clone(),
        InventoryTarget::Client(client_id) => {
            let clients = ctx.clients.read();
            let Some(client) = clients.get(client_id) else {
                panic!("after_inventory_modified: target client {} not found", client_id);
            };
            client.clone()
        }
        InventoryTarget::World(_) => return,
    };

    update_player_skin(&client, ctx.items_manager, commands);
}

fn update_player_skin(client: &Client, items_manager: &SharedItemsManager, commands: &mut Commands) {
    let skin = client.get_player_skin(items_manager);
    commands.queue(UpdatePlayerComponent::_create(
        client.clone(),
        EntityComponent::Skin(Some(EntitySkinComponent::create(skin))),
    ));
}

pub(crate) fn armor_slot_changed(changes: &[InventorySlotChange]) -> bool {
    changes.iter().any(|change| {
        matches!(
            change.slot,
            SPECIAL_INVENTORY_HEAD_SLOT
                | SPECIAL_INVENTORY_CHEST_SLOT
                | SPECIAL_INVENTORY_PANTS_SLOT
                | SPECIAL_INVENTORY_BOOTS_SLOT
        )
    })
}

pub(crate) fn inventory_slot_allowed(slot: u16) -> bool {
    (slot as usize) < INVENTORY_SLOTS
}

pub(crate) fn target_slot_requires_armor(slot: usize) -> bool {
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

pub(crate) fn item_fits_slot(items_manager: &SharedItemsManager, item: &Item, slot: usize) -> bool {
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
