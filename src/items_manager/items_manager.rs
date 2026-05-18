use std::collections::HashMap;

use bevy_ecs::resource::Resource;
use common::inventory::inventory::{ClientInventory, Inventory};
use common::inventory::item::{BodyPart, ClientItem, ClientItemKind, Item, ItemKind, WeaponKind};
use serde::{Deserialize, Serialize};
use strum_macros::Display;

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

#[derive(Display, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Armor {
        body_part: BodyPart,

        // resource png path; For client icon display
        icon: String,

        // Server internal info. resource glb path
        model: String,
    },
    Weapon {
        weapon_kind: WeaponKind,

        // resource png path; For client icon display
        icon: String,

        // Server internal info. resource glb path
        model: String,
    },
}

impl ItemType {
    pub(crate) fn armor(body_part: BodyPart, icon: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Armor {
            body_part,
            icon: icon.into(),
            model: model.into(),
        }
    }

    pub(crate) fn weapon(weapon_kind: WeaponKind, icon: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Weapon {
            weapon_kind,
            icon: icon.into(),
            model: model.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ItemInfo {
    slug: String,
    item_type: ItemType,
    title: String,
    description: String,
}

impl ItemInfo {
    pub(crate) fn create(
        slug: impl Into<String>,
        item_type: ItemType,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            slug: slug.into(),
            item_type,
            title: title.into(),
            description: description.into(),
        }
    }
}

impl ItemsManager {
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

                let icon = match &info.item_type {
                    ItemType::Armor { icon, .. } | ItemType::Weapon { icon, .. } => icon.clone(),
                };

                ClientItem::create(
                    ClientItemKind::Icon(icon.clone()),
                    item.get_amount(),
                    Some(icon),
                    Some(info.title.clone()),
                    Some(info.description.clone()),
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
        let slug = item.slug.clone();

        if self.items.contains_key(&slug) {
            return Err(format!("Item \"{}\" already exists", slug));
        }

        match &item.item_type {
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
        }

        self.items.insert(slug, item);
        Ok(())
    }

    pub(crate) fn has_item(&self, slug: &str) -> bool {
        self.items.contains_key(slug)
    }
}
