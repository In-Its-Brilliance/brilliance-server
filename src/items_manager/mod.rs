pub mod item_info;
pub mod items_manager;

pub use item_info::{ItemInfo, ItemType};

use std::sync::Arc;

use crate::plugins::server_plugin::host_functions::set_items_manager_bridge;
use bevy_app::{App, Plugin};
use bevy_ecs::system::Res;
use common::timed_lock;
use items_manager::{ItemsManager, SharedItemsManager};

#[derive(Default)]
pub struct ItemsManagerPlugin;

impl Plugin for ItemsManagerPlugin {
    fn build(&self, app: &mut App) {
        let items_manager = SharedItemsManager::new(Arc::new(timed_lock!(ItemsManager::default(), "items_manager")));
        app.insert_resource(items_manager);
        app.add_systems(bevy_app::Startup, register_items_manager_bridge);
    }
}

fn register_items_manager_bridge(items_manager: Res<SharedItemsManager>) {
    let _s = crate::span!("items_manager.register_items_manager_bridge");
    set_items_manager_bridge(items_manager.clone_inner());
}
