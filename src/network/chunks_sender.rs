use crate::{
    entities::entity::Position,
    worlds::{world_manager::WorldManager, worlds_manager::WorldsManager},
    CHUNKS_DISTANCE,
};
use bevy_ecs::system::Res;
use common::{
    chunks::{block_position::BlockPositionTrait, chunk_position::ChunkPosition},
    utils::spiral_iterator::SpiralIterator,
};

use super::{
    client_network::{ClientNetwork, WorldEntity},
    clients_container::ClientsContainer,
    server::NetworkContainer,
};

/// Sends missing chunk data to connected clients.
///
/// Iterates over all clients, skips disconnected ones and those with a full send queue,
/// checks the chunks each player is watching, and sends not-yet-sent loaded chunks
/// in the correct order.
pub fn send_chunks(
    worlds_manager: Res<WorldsManager>,
    clients: Res<ClientsContainer>,
    network_container: Res<NetworkContainer>,
) {
    #[cfg(feature = "trace")]
    let _span = bevy_utils::tracing::info_span!("chunks_sender.send_chunks").entered();
    let _s = crate::span!("chunks_sender.send_chunks");

    // iterate over all clients
    for (_client_id, network_client) in clients.iter() {
        // skip disconnected clients
        if !network_container.is_connected(&network_client) {
            continue;
        }

        // skip if send queue is full
        if network_client.is_queue_limit() {
            continue;
        }

        // client is not attached to a world yet
        let Some(world_entity) = network_client.get_world_entity() else {
            continue;
        };

        // get player's world
        let world = worlds_manager.get_world_manager(world_entity.get_world_slug()).unwrap();

        // chunks the player is watching
        let player_watching_chunks = match world.get_chunks_map().get_watching_chunks(&world_entity.get_entity()) {
            Some(v) => v,
            None => continue, // player is not watching any chunks
        };

        // check if there is at least one chunk not sent yet
        let has_not_sent = player_watching_chunks
            .iter()
            .any(|chunk_position| !network_client.is_already_sended(chunk_position));

        // skip if all chanks are sent
        if !has_not_sent {
            continue;
        }

        send_chunks_to_client(&*world, &world_entity, network_client, player_watching_chunks);
    }
}

/// Sends all missing chunks to a single client.
///
/// Iterates over chunks around the player in spiral order,
/// checks that the chunk is watched, loaded, and not yet sent,
/// and enqueues and sends the chunk data until the send queue limit is reached.
fn send_chunks_to_client(
    world_manager: &WorldManager,
    world_entity: &WorldEntity,
    network_client: &ClientNetwork,
    player_watching_chunks: &Vec<ChunkPosition>,
) {
    let ecs = world_manager.get_ecs();
    let entity_ref = ecs.get_entity(world_entity.get_entity()).unwrap();
    let position = entity_ref
        .get::<Position>()
        .expect("player inside world must have position");
    let center = position.get_chunk_position();

    // iterate chunks in spiral order around the player
    let iter = SpiralIterator::new(center.x as i64, center.z as i64, CHUNKS_DISTANCE as i64);

    for (x, z) in iter {
        // stop sending if queue limit is reached
        if network_client.is_queue_limit() {
            return;
        }

        let chunk_position = ChunkPosition::new(x, z);

        // skip chunks the player is not watching
        if !player_watching_chunks.contains(&chunk_position) {
            continue;
        }

        // skip already sent chunks
        if network_client.is_already_sended(&chunk_position) {
            continue;
        }

        // get chunk data
        let chunk = match world_manager.get_chunks_map().get_chunk_column(&chunk_position) {
            Some(c) => c,
            None => continue,
        };
        if !chunk.is_loaded() {
            continue;
        }

        // serialize chunk once and send to client
        let message = world_manager
            .get_network_chunk_bytes(&chunk_position)
            .expect("send_chunks: chunk bytes not found");

        network_client.send_chunk(&chunk_position, message);
        // log::info!(
        //     target: "network.chunks_sender",
        //     "SEND_LOADED_CHUNK {} chunk_position:{}",
        //     world_entity.get_entity().index(),
        //     chunk_position,
        // );
    }
}
