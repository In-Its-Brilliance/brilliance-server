use bevy::prelude::Component;
use network::entities::{EntityNetworkComponent, EntitySkinData};

use super::traits::IEntityNetworkComponent;

#[derive(Component, Clone)]
pub struct EntitySkinComponent {
    skin: EntitySkinData,
}

impl EntitySkinComponent {
    pub fn create(skin: EntitySkinData) -> Self {
        Self { skin }
    }

    pub fn is_generic(&self) -> bool {
        matches!(&self.skin, EntitySkinData::Generic { .. })
    }
}

impl IEntityNetworkComponent for EntitySkinComponent {
    fn to_network(&self) -> EntityNetworkComponent {
        EntityNetworkComponent::Skin(self.skin.clone())
    }

    fn _get_empty() -> EntityNetworkComponent {
        EntityNetworkComponent::Skin(EntitySkinData::None)
    }
}
