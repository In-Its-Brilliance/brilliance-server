use common::chunks::block_position::ChunkBlockPosition;
use common::chunks::chunk_data::{BlockDataInfo, ChunkData};
use common::chunks::chunk_position::ChunkPosition;
use common::chunks::chunk_storage::ChunkStorage;
use common::utils::compressable::Compressable;
use core::fmt;
use network::messages::ServerMessages;
use parking_lot::RwLock;
use std::fmt::Display;
use std::{sync::Arc, time::Duration};

pub struct ChunkColumn {
    chunk_position: ChunkPosition,
    world_slug: String,

    // All chunk data
    chunk_storage: Option<ChunkStorage>,

    despawn_timer: Arc<RwLock<Duration>>,
    loaded: bool,
}

impl Display for ChunkColumn {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ChunkColumn{{x:{} z:{} despawn_timer:{}}}",
            self.chunk_position.x,
            self.chunk_position.z,
            self.despawn_timer.read().as_secs_f32()
        )
    }
}

impl ChunkColumn {
    pub(crate) fn new(chunk_position: ChunkPosition, world_slug: String) -> Self {
        Self {
            chunk_storage: None,
            despawn_timer: Arc::new(RwLock::new(Duration::ZERO)),
            chunk_position,
            world_slug,
            loaded: false,
        }
    }

    pub fn set_chunk_data(&mut self, chunk_storage: ChunkStorage) {
        assert!(
            chunk_storage.get_chunk_data().len() > 0,
            "SET_CHUNK_DATA chunk must contain at least one section"
        );
        self.chunk_storage = Some(chunk_storage);
        self.loaded = true;
        // log::info!(target: "set_sections", "chunk {} loaded", self.chunk_position);
    }

    pub fn get_sections(&self) -> &ChunkData {
        &self.chunk_storage.as_ref().unwrap().get_chunk_data()
    }

    pub fn get_chunk_storage(&self) -> &ChunkStorage {
        self.chunk_storage.as_ref().expect("Chunk storage is not loaded")
    }

    pub fn get_chunk_position(&self) -> &ChunkPosition {
        &self.chunk_position
    }

    /// If chunk load his data
    pub(crate) fn is_loaded(&self) -> bool {
        self.loaded
    }

    pub fn change_block(
        &mut self,
        section: u32,
        chunk_block: &ChunkBlockPosition,
        new_block_info: Option<BlockDataInfo>,
    ) {
        let chunk_data = self.chunk_storage.as_mut().unwrap().get_chunk_data_mut();
        chunk_data.change_block(section, &chunk_block, new_block_info);
    }

    pub(crate) fn is_for_despawn(&self, duration: Duration) -> bool {
        *self.despawn_timer.read() >= duration
    }

    pub(crate) fn set_despawn_timer(&self, new_despawn: Duration) {
        *self.despawn_timer.write() = new_despawn;
    }

    pub(crate) fn increase_despawn_timer(&self, new_despawn: Duration) {
        *self.despawn_timer.write() += new_despawn;
    }

    pub(crate) fn build_network_format(&self) -> ServerMessages {
        assert!(self.loaded, "build_network_format: chunk must be loaded");
        return ServerMessages::ChunkSectionInfoEncoded {
            world_slug: self.world_slug.clone(),
            encoded: self.get_sections().compress(),
            chunk_position: self.chunk_position.clone(),
        };
    }
}
