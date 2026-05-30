mod close;
mod drop;
mod move_ops;

use crate::{
    clients::client::Client,
    clients::clients_container::SharedClientsContainer,
    inventory::inventory_manager::InventoryManager,
    items_manager::items_manager::SharedItemsManager,
    network::events::on_inventory_action::{InventoryAction, InventoryTarget},
    worlds::worlds_manager::SharedWorldsManager,
};

use super::helpers::InventoryActionCtx;

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
        match inventory_target {
            InventoryTarget::Client(target_client_id) if *target_client_id == ctx.client.get_client_id() => Ok(()),
            InventoryTarget::Client(target_client_id) => Err(format!(
                "client {} is not allowed to act on client {} inventory",
                ctx.client.get_client_id(),
                target_client_id
            )),
            InventoryTarget::World(inventory_id) => {
                let Some(world_entity) = ctx.client.get_world_entity() else {
                    return Err(format!(
                        "client {} tried to act on world inventory {} without world entity",
                        ctx.client.get_client_id(),
                        inventory_id
                    ));
                };

                let Some(watchers) = inventory_manager.state().get_inventory_watchers(inventory_id) else {
                    return Err(format!("inventory {} is not watched", inventory_id));
                };

                if !watchers.iter().any(|watcher| *watcher == world_entity.get_entity()) {
                    return Err(format!(
                        "client {} is not watching inventory {}",
                        ctx.client.get_client_id(),
                        inventory_id
                    ));
                }
                Ok(())
            }
        }
    }
}
