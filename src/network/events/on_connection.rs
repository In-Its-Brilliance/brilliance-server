use bevy_ecs::{message::Message, prelude::MessageReader};

use crate::network::client_network::ClientNetwork;

#[derive(Message)]
pub struct PlayerConnectionEvent {
    client: ClientNetwork,
}

impl PlayerConnectionEvent {
    pub fn new(client: ClientNetwork) -> Self {
        Self { client }
    }
}

pub fn on_connection(mut connection_events: MessageReader<PlayerConnectionEvent>) {
    for event in connection_events.read() {
        event.client.send_allow_connection();
    }
}
