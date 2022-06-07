//! # EVM executors
//!
//! Executors are structs that hook gasometer and the EVM core together. It
//! also handles the call stacks in EVM.

mod stack;

pub use self::stack::{StackExecutor, MemoryStackState, StackState, StackSubstateMetadata, StackExitKind};
pub use cyfs_base_meta::evm_def::Log;
