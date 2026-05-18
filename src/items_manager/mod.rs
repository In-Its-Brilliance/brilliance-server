pub mod items_manager;

use std::sync::Arc;

use bevy_app::{App, Plugin};
use bevy_app::Startup;
use bevy_ecs::system::Res;
use common::timed_lock;
use crate::plugins::server_plugin::host_functions::set_items_manager_bridge;
use crate::plugins::plugins_manager::PluginsManager;
use items_manager::{ItemsManager, SharedItemsManager};
use common::inventory::item::{BodyPart, WeaponKind};

#[derive(Default)]
pub struct ItemsManagerPlugin;

impl Plugin for ItemsManagerPlugin {
    fn build(&self, app: &mut App) {
        let items_manager = SharedItemsManager::new(Arc::new(timed_lock!(ItemsManager::default(), "items_manager")));
        app.insert_resource(items_manager);
        app.add_systems(Startup, (register_items_manager_bridge, seed_test_items));
    }
}

fn register_items_manager_bridge(items_manager: Res<SharedItemsManager>) {
    let _s = crate::span!("items_manager.register_items_manager_bridge");
    set_items_manager_bridge(items_manager.clone_inner());
}

fn seed_test_items(items_manager: Res<SharedItemsManager>, plugins_manager: Res<PluginsManager>) {
    let _s = crate::span!("items_manager.seed_test_items");
    let mut items_manager = items_manager.write();
    let plugins_manager = &*plugins_manager;

    let test_armor = items_manager::ItemInfo::create(
        "test_armor",
        items_manager::ItemType::armor(
            BodyPart::Chest,
            "default://assets/gui/textures/elements.png",
            "default://assets/models/generic/generic.glb",
        ),
        "Test Armor",
        "Test armor item",
    );
    if let Err(e) = items_manager.add_item(&plugins_manager, test_armor) {
        log::warn!(target: "items_manager", "failed to add test armor: {}", e);
    }

    let test_sword = items_manager::ItemInfo::create(
        "test_sword",
        items_manager::ItemType::weapon(
            WeaponKind::Sword,
            "default://assets/gui/textures/elements.png",
            "default://assets/models/generic/generic.glb",
        ),
        "Test Sword",
        "Test sword item",
    );
    if let Err(e) = items_manager.add_item(&plugins_manager, test_sword) {
        log::warn!(target: "items_manager", "failed to add test sword: {}", e);
    }
}
