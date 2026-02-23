use bevy_ecs::message::Message;
use bevy_ecs::system::Res;
use common::utils::events::EventReader;

use crate::network::client_network::ClientNetwork;
use crate::network::server::NetworkEventListener;

#[derive(Message)]
pub struct PlayerConnectionEvent {
    client: ClientNetwork,
}

impl PlayerConnectionEvent {
    pub fn new(client: ClientNetwork) -> Self {
        Self { client }
    }
}

pub fn on_connection(connection_events: Res<NetworkEventListener<PlayerConnectionEvent>>) {
    let _s = crate::span!("events.on_connection");
    for event in connection_events.0.iter_events() {
        event.client.send_allow_connection();
    }
}
