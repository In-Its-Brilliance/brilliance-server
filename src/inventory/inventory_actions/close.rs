use crate::{
    inventory::inventory_manager::InventoryManager,
    network::{events::on_inventory_action::InventoryTarget, sync_inventory::send_inventory_stop_to_client},
};

use super::helpers::InventoryActionCtx;

pub(crate) fn apply_close(
    ctx: &InventoryActionCtx<'_>,
    inventory_manager: &mut InventoryManager,
    inventory: InventoryTarget,
) -> Result<(), String> {
    if matches!(&inventory, InventoryTarget::Client(target_client_id) if *target_client_id == ctx.client.get_client_id())
    {
        log::error!(
            target: "inventory",
            "client {} tried to close own inventory",
            ctx.client.get_client_id()
        );
        return Ok(());
    }

    let Some(world_entity) = ctx.client.get_world_entity() else {
        log::error!(
            target: "inventory",
            "client {} tried to close inventory without world entity",
            ctx.client.get_client_id()
        );
        return Ok(());
    };

    match &inventory {
        InventoryTarget::Client(target_client_id) => {
            inventory_manager.close_inventory(world_entity.get_entity(), *target_client_id);
        }
        InventoryTarget::World(inventory_id) => {
            inventory_manager.close_inventory(world_entity.get_entity(), *inventory_id);
        }
    }

    send_inventory_stop_to_client(ctx.client, &inventory);
    Ok(())
}
