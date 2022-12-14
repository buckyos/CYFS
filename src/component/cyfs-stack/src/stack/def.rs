use cyfs_bdt::ChunkReader;
use cyfs_chunk_cache::ChunkManagerRef;
use cyfs_util::*;

use crate::ndn_api::{ChunkStoreReader, ChunkManagerWriter, ChunkWriter};

pub struct NamedDataComponents {
    pub chunk_manager: ChunkManagerRef,
    pub ndc: Box<dyn NamedDataCache>,
    pub tracker: Box<dyn TrackerCache>,
}

impl Clone for NamedDataComponents {
    fn clone(&self) -> Self {
        Self {
            chunk_manager: self.chunk_manager.clone(),
            ndc: self.ndc.clone(),
            tracker: self.tracker.clone(),
        }
    }
}

impl NamedDataComponents {
    pub fn new_chunk_store_reader(&self) -> ChunkStoreReader {
        ChunkStoreReader::new(
            self.chunk_manager.clone(),
            self.ndc.clone(),
            self.tracker.clone(),
        )
    }

    pub fn new_chunk_reader(&self) -> Box<dyn ChunkReader> {
        Box::new(self.new_chunk_store_reader())
    }

    pub fn new_chunk_manager_writer(&self) -> ChunkManagerWriter {
        ChunkManagerWriter::new(
            self.chunk_manager.clone(),
            self.ndc.clone(),
            self.tracker.clone(),
        )
    }

    pub fn new_chunk_writer(&self) -> Box<dyn ChunkWriter> {
        Box::new(self.new_chunk_manager_writer())
    }
}