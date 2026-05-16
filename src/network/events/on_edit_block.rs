use bevy::prelude::ResMut;
use bevy_ecs::message::Message;
use bevy_ecs::system::Res;
use common::chunks::{block_position::BlockPosition, chunk_data::BlockDataInfo};
use common::utils::events::EventReader;
use network::messages::{NetworkMessageType, ServerMessages};

use crate::{
    network::{client_network::ClientNetwork, server::NetworkEventListener, sync_world_change::sync_world_block_change},
    worlds::worlds_manager::WorldsManager,
};

#[derive(Message)]
pub struct EditBlockEvent {
    client: ClientNetwork,
    world_slug: String,
    position: BlockPosition,
    new_block_info: Option<BlockDataInfo>,
}

impl EditBlockEvent {
    pub fn new(
        client: ClientNetwork,
        world_slug: String,
        position: BlockPosition,
        new_block_info: Option<BlockDataInfo>,
    ) -> Self {
        Self {
            client,
            world_slug,
            position,
            new_block_info,
        }
    }
}

pub fn on_edit_block(
    edit_block_events: Res<NetworkEventListener<EditBlockEvent>>,
    worlds_manager: ResMut<WorldsManager>,
) {
    let _s = crate::span!("events.on_edit_block");
    for event in edit_block_events.0.iter_events() {
        let world_entity = event.client.get_world_entity();
        let world_entity = match world_entity.as_ref() {
            Some(w) => w,
            None => {
                log::error!(
                    target: "network",
                    "Client ip:{} tries to request edit block but he is not in the world!",
                    event.client.get_client_ip()
                );
                continue;
            }
        };

        if *world_entity.get_world_slug() != event.world_slug {
            log::error!(
                target: "network",
                "Client ip:{} tries to send edit block from another world!",
                event.client.get_client_ip()
            );
            continue;
        }

        let world_manager = worlds_manager
            .get_world_manager(&world_entity.get_world_slug())
            .unwrap();

        if let Err(e) = world_manager
            .get_chunks_map()
            .edit_block(event.position.clone(), event.new_block_info.clone())
        {
            let msg = ServerMessages::ConsoleOutput { message: e };
            event
                .client
                .send_message(NetworkMessageType::ReliableOrdered, &msg);
            return;
        }
        sync_world_block_change(&*world_manager, event.position, event.new_block_info)
    }
}
