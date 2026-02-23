use bevy_ecs::message::Message;
use bevy_ecs::system::Res;
use common::utils::events::{EventInterface, EventReader};
use network::messages::{NetworkMessageType, ServerMessages};

use crate::network::client_network::ClientNetwork;
use crate::network::events::on_media_loaded::PlayerMediaLoadedEvent;
use crate::network::runtime_plugin::RuntimePlugin;
use crate::network::server::{NetworkEventChannel, NetworkEventListener};
use crate::plugins::plugins_manager::PluginsManager;
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
    events: Res<NetworkEventListener<ResourcesHasCacheEvent>>,
    plugins_manager: Res<PluginsManager>,
    player_media_loaded_channel: Res<NetworkEventChannel<PlayerMediaLoadedEvent>>,
) {
    let _s = crate::span!("events.on_resources_has_cache");
    if RuntimePlugin::is_stopped() {
        return;
    }

    let resources_archive = plugins_manager.get_resources_archive();
    for event in events.0.iter_events() {
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
        player_media_loaded_channel.0.emit_event(msg);
    }
}
