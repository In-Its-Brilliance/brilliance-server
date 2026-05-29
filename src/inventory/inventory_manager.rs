use bevy::prelude::{Entity, Resource};

use crate::{
    clients::client::Client, clients::clients_container::SharedClientsContainer,
    items_manager::items_manager::SharedItemsManager, network::events::on_inventory_action::InventoryAction,
    worlds::worlds_manager::SharedWorldsManager,
};

use super::inventory_actions::InventoryActions;
use super::inventory_state::InventoryState;

#[derive(Default, Resource)]
/// Tracks open inventory viewers and world inventory indices.
/// State only: no network synchronization happens here.
pub struct InventoryManager {
    state: InventoryState,
}

impl InventoryManager {
    /// Registers that a viewer is watching an inventory.
    pub fn open_inventory(&mut self, viewer: Entity, inventory_id: u64) -> bool {
        self.state.watch_inventory(inventory_id, viewer)
    }

    /// Unregisters a viewer from an inventory.
    pub fn close_inventory(&mut self, viewer: Entity, inventory_id: u64) {
        self.state.unwatch_inventory(&inventory_id, &viewer);
    }

    pub(crate) fn state(&self) -> &InventoryState {
        &self.state
    }

    pub(crate) fn state_mut(&mut self) -> &mut InventoryState {
        &mut self.state
    }

    pub fn apply_action(
        &mut self,
        client: &Client,
        action: InventoryAction,
        clients: &SharedClientsContainer,
        items_manager: &SharedItemsManager,
        worlds_manager: &SharedWorldsManager,
    ) -> Result<(), String> {
        InventoryActions::apply_action(client, action, clients, items_manager, self, worlds_manager)
    }
}
