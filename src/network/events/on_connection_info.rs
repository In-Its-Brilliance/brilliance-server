use bevy_ecs::message::Message;
use bevy_ecs::system::Res;
use common::inventory::inventory::InventoryType;
use common::utils::events::{EventInterface, EventReader};
use network::messages::{NetworkMessageType, ServerMessages};

use crate::clients::client::Client;
use crate::clients::client::ClientInfo;
use crate::clients::clients_container::SharedClientsContainer;
use crate::network::events::on_media_loaded::PlayerMediaLoadedEvent;
use crate::network::server::{NetworkEventChannel, NetworkEventListener};
use crate::network::sync_inventory::send_inventory_start_to_client;
use crate::plugins::plugins_manager::PluginsManager;
use crate::runtime_plugin::RuntimePlugin;
use crate::storage::storage_manager::SharedStorageManager;

#[derive(Message)]
pub struct PlayerConnectionInfoEvent {
    client: Client,
    pub login: String,
    pub version: String,
    pub architecture: String,
    pub rendering_device: String,
}

impl PlayerConnectionInfoEvent {
    pub fn new(client: Client, login: String, version: String, architecture: String, rendering_device: String) -> Self {
        Self {
            client,
            login,
            version,
            architecture,
            rendering_device,
        }
    }
}

pub fn on_connection_info(
    connection_info_events: Res<NetworkEventListener<PlayerConnectionInfoEvent>>,
    plugins_manager: Res<PluginsManager>,
    player_media_loaded_channel: Res<NetworkEventChannel<PlayerMediaLoadedEvent>>,
    clients: Res<SharedClientsContainer>,
    storage: Res<SharedStorageManager>,
) {
    let _s = crate::span!("events.on_connection_info");
    if RuntimePlugin::is_stopped() {
        return;
    }

    for event in connection_info_events.0.iter_events() {
        let clients_guard = clients.read();
        for (client_id, client) in clients_guard.iter() {
            if *client_id != event.client.get_client_id() {
                if let Some(client_info) = client.get_client_info() {
                    if *client_info.get_login() == event.login {
                        // This login already connected
                        let message = "This account is already logged in from another session.";
                        event.client.disconnect(Some(message.to_string()));
                        return;
                    }
                }
            }
        }

        let client_info = ClientInfo::new(&event);
        event.client.set_client_info(client_info.clone());
        let storage_guard = storage.read();
        if let Err(e) = event.client.read_player_data(&storage_guard.read_server_storage()) {
            log::error!(target: "storage", "&cFailed to load player data for &4{}&c: {}", client_info.get_login(), e);
            event.client.disconnect(Some("Failed to load player data".to_string()));
            return;
        };

        if let Some(player_data) = event.client.get_player_data() {
            send_inventory_start_to_client(
                &event.client,
                InventoryType::PlayerPersonal,
                player_data.get_inventory().to_client_inventory(),
            );
        }

        log::info!(
            target: "network",
            "&a✱ {login}&r connected &7ip: &e{ip} &7id: &8{id}&r &7version: &8{version}",
            login = client_info.get_login(),
            ip = event.client.get_client_ip(),
            id = event.client.get_client_id(),
            version = client_info.get_version(),
        );

        let resources_archive = plugins_manager.get_resources_archive();
        if resources_archive.has_any() {
            // Sending resources schema if necessary

            let scheme = ServerMessages::ResourcesScheme {
                list: resources_archive.get_resources_scheme().clone(),
                archive_hash: resources_archive.get_archive_hash().clone(),
            };
            event.client.send_message(NetworkMessageType::ReliableOrdered, &scheme);
        } else {
            // Or send player as loaded

            let msg = PlayerMediaLoadedEvent::new(event.client.clone(), None);
            player_media_loaded_channel.0.emit_event(msg);
        }
    }
}
