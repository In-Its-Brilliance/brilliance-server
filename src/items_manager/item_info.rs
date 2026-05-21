use serde::{Deserialize, Serialize};
use strum_macros::Display;

use common::inventory::item::{BodyPart, WeaponKind};

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
    Other {
        icon: String,
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

    pub(crate) fn other(icon: impl Into<String>) -> Self {
        Self::Other { icon: icon.into() }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ItemInfo {
    slug: String,
    item_type: ItemType,
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
        title: impl Into<String>,
        description: impl Into<String>,
        max_stack_size: u16,
    ) -> Self {
        assert!(max_stack_size >= 1, "max_stack_size must be at least 1");
        Self {
            slug: slug.into(),
            item_type,
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
