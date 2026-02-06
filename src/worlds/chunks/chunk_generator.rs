use std::sync::Arc;

use common::{
    chunks::{chunk_data::ChunkData, chunk_position::ChunkPosition},
    world_generator::{default::WorldGenerator, traits::IWorldGenerator},
    worlds_storage::taits::IWorldStorage,
};
use parking_lot::RwLock;

use super::{chunk_column::ChunkColumn, chunks_map::StorageLock};
use crate::network::runtime_plugin::RuntimePlugin;

pub(crate) fn load_chunk(
    world_generator: Arc<RwLock<WorldGenerator>>,
    storage: StorageLock,
    chunk_position: ChunkPosition,
    chunk_column: Arc<RwLock<ChunkColumn>>,
    loaded_chunks: flume::Sender<ChunkPosition>,
) {
    rayon::spawn(move || {
        #[cfg(feature = "trace")]
        let _span = bevy_utils::tracing::info_span!("chunk_column.load_chunk").entered();
        let _s = crate::span!("chunk_column.load_chunk");

        if RuntimePlugin::is_stopped() {
            return;
        }

        // Load from storage
        let index = match storage.read().has_chunk_data(&chunk_position) {
            Ok(i) => i,
            Err(e) => {
                log::error!(target: "worlds", "&cChunk load error!");
                log::error!(target: "worlds", "Error: {}", e);
                RuntimePlugin::stop();
                return;
            }
        };

        let sections = if let Some(index) = index {
            let encoded = match storage.read().read_chunk_data(index) {
                Ok(c) => c,
                Err(e) => {
                    log::error!(target: "worlds", "&cChunk load error!");
                    log::error!(target: "worlds", "Error: {}", e);
                    RuntimePlugin::stop();
                    return;
                }
            };
            let encoded_len = encoded.len();
            match ChunkData::decompress(encoded) {
                Ok(d) => d,
                Err(e) => {
                    log::error!(target: "worlds", "&cChunk decode error!");
                    log::error!(target: "worlds", "Error: {} (encoded size:{})", e, encoded_len);
                    RuntimePlugin::stop();
                    return;
                }
            }
        }
        // Or generate new
        else {
            world_generator.read().generate_chunk_data(&chunk_position)
        };
        let mut chunk_column = chunk_column.write();
        chunk_column.set_sections(sections);

        if !cfg!(test) {
            loaded_chunks.send(chunk_position).expect("channel poisoned");
        }
    })
}
