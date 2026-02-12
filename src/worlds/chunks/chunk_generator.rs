use parking_lot::RwLock;
use std::sync::Arc;

use common::{
    chunks::{chunk_data::ChunkData, chunk_position::ChunkPosition},
    plugin_api::events::generage_chunk::ChunkGenerateEvent,
    utils::compressable::Compressable,
    world_generator::traits::WorldGeneratorSettings,
    worlds_storage::taits::IWorldStorage,
};

use super::{chunk_column::ChunkColumn, chunks_map::StorageLock};
use crate::{network::runtime_plugin::RuntimePlugin, plugins::server_plugin::plugin_instance::WASMPluginManager};

pub(crate) fn load_chunk(
    plugin: Arc<WASMPluginManager>,
    world_generator_settings: WorldGeneratorSettings,
    storage: StorageLock,
    chunk_position: ChunkPosition,
    chunk_column: Arc<RwLock<ChunkColumn>>,
    loaded_chunks: flume::Sender<ChunkPosition>,
) {
    rayon::spawn(move || {
        if RuntimePlugin::is_stopped() {
            return;
        }

        #[cfg(feature = "trace")]
        let _span = bevy_utils::tracing::info_span!("chunk_column.load_chunk").entered();
        let _s = crate::span!("chunk_column.load_chunk");

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
            let event = ChunkGenerateEvent::create(chunk_position, world_generator_settings);
            match plugin.call_event_with_result(&event) {
                Ok(sections) => sections,
                Err(e) => {
                    log::error!(target: "worlds", "&4Chunk generation error: &c{}", e);
                    RuntimePlugin::stop();
                    return;
                }
            }
        };
        let mut chunk_column = chunk_column.write();
        chunk_column.set_sections(sections);

        if !cfg!(test) {
            loaded_chunks.send(chunk_position).expect("channel poisoned");
        }
    })
}
