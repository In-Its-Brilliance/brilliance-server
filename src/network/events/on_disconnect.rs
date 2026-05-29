use bevy_ecs::{message::Message, system::Res};
use common::utils::events::EventReader;

use crate::{
    clients::{client::Client, clients_container::SharedClientsContainer},
    entities::skin::EntitySkinComponent,
    inventory::SharedInventoryManager,
    network::{server::NetworkEventListener, sync_entities::sync_entity_despawn},
    storage::storage_manager::SharedStorageManager,
    worlds::worlds_manager::SharedWorldsManager,
};

#[derive(Message)]
pub struct PlayerDisconnectEvent {
    client: Client,
    reason: String,
}

impl PlayerDisconnectEvent {
    pub fn new(client: Client, reason: String) -> Self {
        Self { client, reason }
    }
}

pub fn on_disconnect(
    disconnection_events: Res<NetworkEventListener<PlayerDisconnectEvent>>,
    clients: Res<SharedClientsContainer>,
    inventory_manager: Res<SharedInventoryManager>,
    storage: Res<SharedStorageManager>,
    worlds_manager: Res<SharedWorldsManager>,
) {
    let _s = crate::span!("events.on_disconnect");
    for event in disconnection_events.0.iter_events() {
        if let Some(i) = event.client.get_client_info() {
            log::info!(
                target: "network",
                "&o✗ {login}&r disconnected &8reason: &7{reason}",
                login = i.get_login(),
                reason = event.reason,
            );
        }

        let storage_guard = storage.read();
        if let Err(e) = event.client.save_player_data(&storage_guard.read_server_storage()) {
            log::error!(target: "storage", "&cFailed to save player data for client &4{}&c: {}", event.client.get_client_id(), e);
        }

        // Check if player was in the world, despawn if so
        let world_entity = event.client.get_world_entity();
        match world_entity {
            Some(c) => {
                let worlds_manager_guard = worlds_manager.read();
                let mut world_manager = worlds_manager_guard.get_world_manager_mut(&c.get_world_slug()).unwrap();
                let mut inventory_manager = inventory_manager.write();

                let ecs = world_manager.get_ecs();
                let entity_ref = ecs.get_entity(c.get_entity()).unwrap();

                // Sync his entity if exists
                if entity_ref.get::<EntitySkinComponent>().is_some() {
                    sync_entity_despawn(&*world_manager, c.get_entity());
                }

                world_manager.despawn_player(&c, &mut inventory_manager);
            }
            None => (),
        };
        clients.write().remove(&event.client.get_client_id());
    }
}
