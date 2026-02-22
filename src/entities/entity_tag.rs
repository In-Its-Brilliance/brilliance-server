use bevy::prelude::Component;
use network::entities::{entity_tag::EntityTagData, EntityNetworkComponent};

use super::traits::IEntityNetworkComponent;

#[derive(Component, Clone)]
pub struct EntityTagComponent(EntityTagData);

impl EntityTagComponent {
    pub fn create(tag: EntityTagData) -> Self {
        Self(tag)
    }
}

impl IEntityNetworkComponent for EntityTagComponent {
    fn to_network(&self) -> EntityNetworkComponent {
        EntityNetworkComponent::Tag(Some(self.0.clone()))
    }

    fn get_empty() -> EntityNetworkComponent {
        EntityNetworkComponent::Tag(None)
    }
}
