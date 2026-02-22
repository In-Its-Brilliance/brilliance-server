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
    let _s = crate::span!("events.on_connection");
    for event in connection_events.read() {
        event.client.send_allow_connection();
    }
}
