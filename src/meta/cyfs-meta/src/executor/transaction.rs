use crate::state_storage::{StateRef, StateWeakRef};
use super::context;
use crate::executor::context::{ConfigRef, ConfigWeakRef};
use crate::BlockDesc;
use crate::events::event_manager::{EventManagerWeakRef, EventManagerRef};
use crate::archive_storage::{ArchiveRef, ArchiveWeakRef};

pub struct ExecuteContext {
    ref_state: StateWeakRef,
    ref_archive: ArchiveWeakRef,
    block: BlockDesc,
    caller: context::Account,
    config: ConfigWeakRef,
    event_manager: EventManagerWeakRef,
    is_verify_block: bool
}

impl ExecuteContext {
    pub fn new(ref_state: &StateRef, ref_archive: &ArchiveRef, block: &BlockDesc, caller: context::Account, config: &ConfigRef, event_manager: &EventManagerRef, is_verify_block: bool) -> ExecuteContext {
        ExecuteContext {
            ref_state: StateRef::downgrade(ref_state),
            ref_archive: ArchiveRef::downgrade(ref_archive),
            block: block.clone(),
            caller,
            config: ConfigRef::downgrade(config),
            event_manager: EventManagerRef::downgrade(event_manager),
            is_verify_block
        }
    }
    pub fn block(&self) -> &BlockDesc {
        &self.block
    }

    pub fn caller(&mut self) -> &mut context::Account {
        &mut self.caller
    }

    pub fn ref_state(&self) -> &StateWeakRef {
        &self.ref_state
    }

    pub fn ref_archive(&self) -> &ArchiveWeakRef {
        &self.ref_archive
    }

    pub fn config(&self) -> &ConfigWeakRef {
        &self.config
    }

    pub fn event_manager(&self) -> &EventManagerWeakRef {
        &self.event_manager
    }

    pub fn is_verify_block(&self) -> bool {
        self.is_verify_block
    }
}
