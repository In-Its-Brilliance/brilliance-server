use bevy_ecs::message::Message;
use bevy_ecs::message::MessageReader;
use bevy_ecs::message::MessageWriter;
use bevy_ecs::system::Res;
use network::messages::{NetworkMessageType, ServerMessages};

use crate::client_resources::resources_manager::ResourceManager;
use crate::network::client_network::ClientInfo;
use crate::network::client_network::ClientNetwork;
use crate::network::clients_container::ClientsContainer;
use crate::network::events::on_media_loaded::PlayerMediaLoadedEvent;

#[derive(Message)]
pub struct PlayerConnectionInfoEvent {
    client: ClientNetwork,
    pub login: String,
    pub version: String,
    pub architecture: String,
    pub rendering_device: String,
}

impl PlayerConnectionInfoEvent {
    pub fn new(
        client: ClientNetwork,
        login: String,
        version: String,
        architecture: String,
        rendering_device: String,
    ) -> Self {
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
    mut connection_info_events: MessageReader<PlayerConnectionInfoEvent>,
    resources_manager: Res<ResourceManager>,
    mut player_media_loaded_events: MessageWriter<PlayerMediaLoadedEvent>,
    clients: Res<ClientsContainer>,
) {
    for event in connection_info_events.read() {

        for (client_id, client) in clients.iter() {
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

        log::info!(
            target: "network",
            "&a✱ {login}&r connected &7ip: &e{ip} &7id: &8{id}&r &7version: &8{version}",
            login = client_info.get_login(),
            ip = event.client.get_client_ip(),
            id = event.client.get_client_id(),
            version = client_info.get_version(),
        );

        if resources_manager.has_any_resources() {
            // Sending resources schema if necessary

            let scheme = ServerMessages::ResourcesScheme {
                list: resources_manager.get_resources_scheme().clone(),
                archive_hash: resources_manager.get_archive_hash().clone(),
            };
            event.client.send_message(NetworkMessageType::ReliableOrdered, &scheme);
        } else {
            // Or send player as loaded

            let msg = PlayerMediaLoadedEvent::new(event.client.clone(), None);
            player_media_loaded_events.write(msg);
        }
    }
}
