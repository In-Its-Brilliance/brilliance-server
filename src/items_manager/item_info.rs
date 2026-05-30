use serde::{Deserialize, Serialize};
use strum_macros::Display;

use common::inventory::item::{BodyPart, WeaponKind};

#[derive(Display, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ItemDisplay {
    // resource png path; For client icon display
    Icon(String),
}

#[derive(Display, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Armor {
        body_part: BodyPart,

        // Server internal info. resource glb path
        model: String,
    },
    Weapon {
        weapon_kind: WeaponKind,

        // Server internal info. resource glb path
        model: String,
    },
    Neck,
    Bracer,
    Belt,
    Ring,
    Other,
}

impl ItemType {
    pub(crate) fn armor(body_part: BodyPart, model: impl Into<String>) -> Self {
        Self::Armor {
            body_part,
            model: model.into(),
        }
    }

    pub(crate) fn weapon(weapon_kind: WeaponKind, model: impl Into<String>) -> Self {
        Self::Weapon {
            weapon_kind,
            model: model.into(),
        }
    }

    pub(crate) fn other() -> Self {
        Self::Other
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ItemInfo {
    slug: String,
    item_type: ItemType,
    item_display: ItemDisplay,
    title: String,
    description: String,
    #[serde(default = "ItemInfo::default_max_stack_size")]
    max_stack_size: u16,
}

impl ItemInfo {
    fn default_max_stack_size() -> u16 {
        1
    }

    pub(crate) fn create(
        slug: impl Into<String>,
        item_type: ItemType,
        item_display: ItemDisplay,
        title: impl Into<String>,
        description: impl Into<String>,
        max_stack_size: u16,
    ) -> Self {
        assert!(max_stack_size >= 1, "max_stack_size must be at least 1");
        Self {
            slug: slug.into(),
            item_type,
            item_display,
            title: title.into(),
            description: description.into(),
            max_stack_size,
        }
    }

    pub(crate) fn slug(&self) -> &String {
        &self.slug
    }

    pub(crate) fn item_type(&self) -> &ItemType {
        &self.item_type
    }

    pub(crate) fn item_display(&self) -> &ItemDisplay {
        &self.item_display
    }

    pub(crate) fn title(&self) -> &String {
        &self.title
    }

    pub(crate) fn description(&self) -> &String {
        &self.description
    }

    pub(crate) fn max_stack_size(&self) -> u16 {
        self.max_stack_size
    }
}
