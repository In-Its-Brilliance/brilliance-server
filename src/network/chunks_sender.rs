use crate::{
    entities::entity::Position,
    worlds::{world_manager::WorldManager, worlds_manager::WorldsManager},
    CHUNKS_DISTANCE,
};
use bevy_ecs::{resource::Resource, system::Res};
use common::{
    chunks::{block_position::BlockPositionTrait, chunk_position::ChunkPosition},
    utils::spiral_iterator::SpiralIterator,
};
use network::messages::{NetworkMessageType, ServerMessages};

use super::{
    client_network::{ClientNetwork, WorldEntity},
    clients_container::ClientsContainer,
    server::NetworkContainer,
};

pub struct PreparedChunk {
    client_id: u64,
    message: ServerMessages,
}

#[derive(Resource)]
pub struct ChunkCompressQueue {
    sender: flume::Sender<PreparedChunk>,
    receiver: flume::Receiver<PreparedChunk>,
}

impl Default for ChunkCompressQueue {
    fn default() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self { sender, receiver }
    }
}

/// Sends missing chunk data to connected clients.
///
/// Iterates over all clients, skips disconnected ones and those with a full send queue,
/// checks the chunks each player is watching, and sends not-yet-sent loaded chunks
/// in the correct order.
pub fn send_chunks(
    worlds_manager: Res<WorldsManager>,
    clients: Res<ClientsContainer>,
    network_container: Res<NetworkContainer>,
    compress_queue: Res<ChunkCompressQueue>,
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

        send_chunks_to_client(&*world, &world_entity, network_client, player_watching_chunks, &compress_queue.sender);
    }
}

/// Sends all missing chunks to a single client.
///
/// Iterates over chunks around the player in spiral order,
/// checks that the chunk is watched, loaded, and not yet sent,
/// marks the chunk as queued and spawns a rayon task for zstd compression.
fn send_chunks_to_client(
    world_manager: &WorldManager,
    world_entity: &WorldEntity,
    network_client: &ClientNetwork,
    player_watching_chunks: &Vec<ChunkPosition>,
    sender: &flume::Sender<PreparedChunk>,
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

        // get chunk Arc for rayon task
        let chunk_arc = match world_manager.get_chunks_map().get_chunk_column_arc(&chunk_position) {
            Some(c) => c,
            None => continue,
        };

        // check loaded before spawning
        if !chunk_arc.read().is_loaded() {
            continue;
        }

        // mark as queued immediately to prevent re-picking
        network_client.mark_chunk_sending(&chunk_position);

        // spawn compression task in rayon threadpool
        let sender = sender.clone();
        let client_id = network_client.get_client_id();
        rayon::spawn(move || {
            let chunk = chunk_arc.read();
            let message = chunk.build_network_format();
            let _ = sender.send(PreparedChunk {
                client_id,
                message,
            });
        });
    }
}

/// Drains compressed chunk results from rayon and sends them over the network.
pub fn flush_compressed_chunks(
    compress_queue: Res<ChunkCompressQueue>,
    clients: Res<ClientsContainer>,
    network_container: Res<NetworkContainer>,
) {
    let _s = crate::span!("chunks_sender.flush_compressed_chunks");

    for prepared in compress_queue.receiver.drain() {
        let Some(client) = clients.get(&prepared.client_id) else {
            continue;
        };
        if !network_container.is_connected(client) {
            continue;
        }
        client.send_message(NetworkMessageType::WorldInfo, &prepared.message);
    }
}
