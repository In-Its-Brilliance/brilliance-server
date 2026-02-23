use bevy::prelude::Res;
use bevy_ecs::message::Message;
use common::utils::events::EventReader;
use network::messages::{NetworkMessageType, ServerMessages};

use crate::{
    network::{client_network::ClientNetwork, runtime_plugin::RuntimePlugin, server::NetworkEventListener},
    plugins::{plugins_manager::PluginsManager, resources_archive::ARCHIVE_CHUNK_SIZE, server_settings::ServerSettings},
};

#[derive(Message)]
pub struct PlayerMediaLoadedEvent {
    client: ClientNetwork,
    last_index: Option<u32>,
}

/// Event to confirm data download
///
/// last_index is last downloaded index part
impl PlayerMediaLoadedEvent {
    pub fn new(client: ClientNetwork, last_index: Option<u32>) -> Self {
        Self { client, last_index }
    }
}

pub fn on_media_loaded(
    events: Res<NetworkEventListener<PlayerMediaLoadedEvent>>,
    server_settings: Res<ServerSettings>,
    plugins_manager: Res<PluginsManager>,
) {
    let _s = crate::span!("events.on_media_loaded");
    if RuntimePlugin::is_stopped() {
        return;
    }

    let resources_archive = plugins_manager.get_resources_archive();
    for event in events.0.iter_events() {
        match event.last_index {
            Some(index) => {
                let total = resources_archive.get_archive_parts_count(ARCHIVE_CHUNK_SIZE);
                let is_last = (index as usize) + 1 >= total;
                if !is_last {
                    // Send new media part
                    let resources_part = ServerMessages::ResourcesPart {
                        index: index + 1,
                        total: total as u32,
                        data: resources_archive.get_archive_part(index as usize + 1, ARCHIVE_CHUNK_SIZE),
                    };

                    event
                        .client
                        .send_message(NetworkMessageType::ReliableUnordered, &resources_part);
                    return;
                }
            }
            None => (),
        }

        // Send server settings
        event.client.send_message(
            NetworkMessageType::ReliableOrdered,
            &server_settings.get_network_settings(),
        );
    }
}
