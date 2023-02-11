use crate::cache::{ChunkManagerWriter, ChunkStoreReader, ChunkWriter};
use crate::context::ContextManager;
use cyfs_bdt::{ChunkReader, StackGuard};
use cyfs_chunk_cache::ChunkManagerRef;
use cyfs_util::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;

pub struct NamedDataComponents {
    pub bdt_stack: Arc<OnceCell<StackGuard>>,
    pub chunk_manager: ChunkManagerRef,
    pub ndc: Box<dyn NamedDataCache>,
    pub tracker: Box<dyn TrackerCache>,
    pub context_manager: ContextManager,
}

impl Clone for NamedDataComponents {
    fn clone(&self) -> Self {
        Self {
            bdt_stack: self.bdt_stack.clone(),
            chunk_manager: self.chunk_manager.clone(),
            ndc: self.ndc.clone(),
            tracker: self.tracker.clone(),
            context_manager: self.context_manager.clone(),
        }
    }
}

impl NamedDataComponents {
    pub(crate) fn new(
        chunk_manager: ChunkManagerRef,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        context_manager: ContextManager,
    ) -> Self {
        Self {
            bdt_stack: Arc::new(OnceCell::new()),
            chunk_manager,
            ndc,
            tracker,
            context_manager,
        }
    }

    pub fn bind_bdt_stack(&self, bdt_stack: StackGuard) {
        if let Err(_) = self.bdt_stack.set(bdt_stack) {
            unreachable!();
        }
    }

    pub fn bdt_stack(&self) -> &StackGuard {
        self.bdt_stack.get().unwrap()
    }

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
