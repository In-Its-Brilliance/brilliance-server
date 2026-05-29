use bevy_ecs::change_detection::Mut;
use bevy_ecs::message::Message;
use bevy_ecs::world::World;
use common::commands::complitions::CompleteRequest;
use common::utils::events::EventReader;
use network::messages::{NetworkMessageType, ServerMessages};

use crate::{
    clients::client::Client, console::commands_executer::CommandsHandler, network::server::NetworkEventListener,
};

#[derive(Message)]
pub struct ConsoleCompleteRequestEvent {
    client: Client,
    request: CompleteRequest,
}

impl ConsoleCompleteRequestEvent {
    pub fn new(client: Client, request: CompleteRequest) -> Self {
        Self { client, request }
    }
}

pub fn on_console_complete(world: &mut World) {
    let _s = crate::span!("events.on_console_complete");
    world.resource_scope(
        |world, events: Mut<NetworkEventListener<ConsoleCompleteRequestEvent>>| {
            for event in events.0.iter_events() {
                let response = CommandsHandler::complete(world, Box::new(event.client.clone()), &event.request);
                event.client.send_message(
                    NetworkMessageType::ReliableOrdered,
                    &ServerMessages::ConsoleCompleteResponse(response),
                );
            }
        },
    );
}
