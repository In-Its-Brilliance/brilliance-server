use bevy_ecs::message::Message;
use bevy_ecs::message::MessageReader;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Res;
use network::messages::{NetworkMessageType, ServerMessages};

use crate::network::client_network::ClientNetwork;
use crate::network::events::on_media_loaded::PlayerMediaLoadedEvent;
use crate::plugins::plugin_manager::PluginManager;
use crate::plugins::resources_archive::ARCHIVE_CHUNK_SIZE;

#[derive(Message)]
pub struct ResourcesHasCacheEvent {
    client: ClientNetwork,
    exists: bool,
}

impl ResourcesHasCacheEvent {
    pub fn new(client: ClientNetwork, exists: bool) -> Self {
        Self { client, exists }
    }
}

pub fn on_resources_has_cache(
    mut events: MessageReader<ResourcesHasCacheEvent>,
    plugin_manager: Res<PluginManager>,
    mut player_media_loaded_events: MessageWriter<PlayerMediaLoadedEvent>,
) {
    let resources_archive = plugin_manager.get_resources_archive();
    for event in events.read() {
        if !event.exists {
            if resources_archive.has_any() {
                // Send first part of archive
                let total = resources_archive.get_archive_parts_count(ARCHIVE_CHUNK_SIZE);
                let resources_part = ServerMessages::ResourcesPart {
                    index: 0,
                    total: total as u32,
                    data: resources_archive.get_archive_part(0, ARCHIVE_CHUNK_SIZE),
                };
                event
                    .client
                    .send_message(NetworkMessageType::ReliableUnordered, &resources_part);
                return;
            }
        }
        // Or send player as loaded
        let msg = PlayerMediaLoadedEvent::new(event.client.clone(), None);
        player_media_loaded_events.write(msg);
    }
}
