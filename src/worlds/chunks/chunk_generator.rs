use parking_lot::RwLock;
use std::sync::Arc;

use common::{
    chunks::{chunk_data::ChunkData, chunk_position::ChunkPosition, chunk_storage::ChunkStorage},
    plugin_api::events::generage_chunk::ChunkGenerateEvent,
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

        let chunk_storage = if let Some(index) = index {
            match storage.read().read_chunk_data(index) {
                Ok(c) => c,
                Err(e) => {
                    log::error!(target: "worlds", "&cChunk load error!");
                    log::error!(target: "worlds", "Error: {}", e);
                    RuntimePlugin::stop();
                    return;
                }
            }
        }
        // Or generate new
        else {
            let event = ChunkGenerateEvent::create(chunk_position, world_generator_settings);
            let chunk_data: ChunkData = match plugin.call_event_with_result(&event) {
                Ok(sections) => sections,
                Err(e) => {
                    log::error!(target: "worlds", "&4Chunk generation error: &c{}", e);
                    RuntimePlugin::stop();
                    return;
                }
            };
            ChunkStorage::create(chunk_data)
        };
        let mut chunk_column = chunk_column.write();
        chunk_column.set_chunk_data(chunk_storage);

        if !cfg!(test) {
            loaded_chunks.send(chunk_position).expect("channel poisoned");
        }
    })
}
