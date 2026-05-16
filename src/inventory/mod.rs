pub mod commands;
pub mod inventory_actions;
pub mod inventory_load_state;
pub mod inventory_manager;
pub mod inventory_state;

use bevy_app::{App, Plugin, Startup};
use common::timed_lock;
use std::sync::Arc;

use crate::{
    plugins::server_plugin::host_functions::set_inventory_manager_bridge,
    utils::Shared,
};

pub type SharedInventoryManager = Shared<inventory_manager::InventoryManager>;

#[derive(Default)]
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        let inventory_manager = SharedInventoryManager::new(Arc::new(timed_lock!(
            inventory_manager::InventoryManager::default(),
            "inventory_manager"
        )));
        app.insert_resource(inventory_manager);
        app.add_systems(Startup, register_inventory_manager_bridge);
    }
}

fn register_inventory_manager_bridge(inventory_manager: bevy_ecs::system::Res<SharedInventoryManager>) {
    let _s = crate::span!("inventory.register_inventory_manager_bridge");
    set_inventory_manager_bridge(inventory_manager.clone_inner());
}
