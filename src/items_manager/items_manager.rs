use std::collections::HashMap;

use bevy_ecs::resource::Resource;
use common::inventory::inventory::{ClientInventory, Inventory};
use common::inventory::item::{ClientItem, ClientItemKind, Item, ItemKind};

use super::item_info::{ItemInfo, ItemType};
use crate::plugins::plugins_manager::PluginsManager;
use crate::utils::Shared;

pub type SharedItemsManager = Shared<ItemsManager>;

#[derive(Resource)]
pub struct ItemsManager {
    items: HashMap<String, ItemInfo>,
}

impl Default for ItemsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemsManager {
    const BLOCK_MAX_STACK_SIZE: u16 = 64;

    pub(crate) fn new() -> Self {
        Self { items: HashMap::new() }
    }

    pub(crate) fn to_client_item(&self, item: &Item) -> ClientItem {
        match item.get_item_kind() {
            ItemKind::Block(block_id) => {
                ClientItem::create(ClientItemKind::Block(*block_id), item.get_amount(), None, None, None)
            }
            ItemKind::CustomItem(slug) => {
                let Some(info) = self.items.get(slug) else {
                    return ClientItem::create(
                        ClientItemKind::Icon(slug.clone()),
                        item.get_amount(),
                        None,
                        Some(slug.clone()),
                        None,
                    );
                };

                let icon = match info.item_type() {
                    ItemType::Armor { icon, .. } | ItemType::Weapon { icon, .. } => icon.clone(),
                    ItemType::Other { icon } => icon.clone(),
                };

                ClientItem::create(
                    ClientItemKind::Icon(icon.clone()),
                    item.get_amount(),
                    Some(icon),
                    match info.item_type() {
                        ItemType::Other { .. } => None,
                        _ => Some(info.title().clone()),
                    },
                    match info.item_type() {
                        ItemType::Other { .. } => None,
                        _ => Some(info.description().clone()),
                    },
                )
            }
        }
    }

    pub(crate) fn to_client_inventory(&self, inventory: &Inventory) -> ClientInventory {
        ClientInventory {
            slots: inventory
                .iter_slots()
                .map(|slot| slot.as_ref().map(|item| self.to_client_item(item)))
                .collect(),
        }
    }

    pub(crate) fn add_item(&mut self, plugins: &PluginsManager, item: ItemInfo) -> Result<(), String> {
        let slug = item.slug().clone();

        if self.items.contains_key(&slug) {
            return Err(format!("Item \"{}\" already exists", slug));
        }

        match item.item_type() {
            ItemType::Armor { icon, model, .. } | ItemType::Weapon { icon, model, .. } => {
                if let Err(e) = plugins.has_media(icon) {
                    return Err(format!("Item \"{}\" icon media not found: \"{}\" ({})", slug, icon, e));
                }
                if let Err(e) = plugins.has_media(model) {
                    return Err(format!(
                        "Item \"{}\" model media not found: \"{}\" ({})",
                        slug, model, e
                    ));
                }
            }
            ItemType::Other { icon } => {
                if let Err(e) = plugins.has_media(icon) {
                    return Err(format!("Item \"{}\" icon media not found: \"{}\" ({})", slug, icon, e));
                }
            }
        }

        self.items.insert(slug, item);
        Ok(())
    }

    pub(crate) fn get_max_stack_size(&self, item: &Item) -> u16 {
        match item.get_item_kind() {
            ItemKind::Block(_) => Self::BLOCK_MAX_STACK_SIZE,
            ItemKind::CustomItem(slug) => self.items.get(slug).map(ItemInfo::max_stack_size).unwrap_or(1),
        }
    }

    pub(crate) fn has_item(&self, slug: &str) -> bool {
        self.items.contains_key(slug)
    }

    pub(crate) fn iter_slugs(&self) -> impl Iterator<Item = &String> {
        self.items.keys()
    }
}
