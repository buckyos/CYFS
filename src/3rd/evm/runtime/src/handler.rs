use alloc::vec::Vec;
use primitive_types::{H256, U256};
use crate::{Capture, Stack, ExitError, Opcode,
			CreateScheme, Context, Machine, ExitReason};
use cyfs_base::ObjectId;

/// Transfer from source to target, with given value.
#[derive(Clone, Debug)]
pub struct Transfer {
	/// Source address.
	pub source: ObjectId,
	/// Target address.
	pub target: ObjectId,
	/// Transfer value.
	pub value: u64,
}

/// EVM context handler.
pub trait Handler {
	/// Type of `CREATE` interrupt.
	type CreateInterrupt;
	/// Feedback value for `CREATE` interrupt.
	type CreateFeedback;
	/// Type of `CALL` interrupt.
	type CallInterrupt;
	/// Feedback value of `CALL` interrupt.
	type CallFeedback;

	/// Get balance of address.
	fn balance(&self, address: ObjectId) -> u64;
	/// Get code size of address.
	fn code_size(&self, address: ObjectId) -> U256;
	/// Get code hash of address.
	fn code_hash(&self, address: ObjectId) -> H256;
	/// Get code of address.
	fn code(&self, address: ObjectId) -> Vec<u8>;
	/// Get storage value of address at index.
	fn storage(&self, address: ObjectId, index: H256) -> H256;
	/// Get original storage value of address at index.
	fn original_storage(&self, address: ObjectId, index: H256) -> H256;

	/// Get the gas left value.
	fn gas_left(&self) -> U256;
	/// Get the gas price value.
	fn gas_price(&self) -> U256;
	/// Get execution origin.
	fn origin(&self) -> ObjectId;
	fn block_coinbase(&self) -> ObjectId;
	/// Get environmental block hash.
	fn block_hash(&self, number: U256) -> H256;
	/// Get environmental block number.
	fn block_number(&self) -> U256;
	/// Get environmental block timestamp.
	fn block_timestamp(&self) -> U256;
	/// Get environmental gas limit.
	fn block_gas_limit(&self) -> U256;
	/// Get environmental chain ID.
	fn chain_id(&self) -> U256;

	/// Check whether an address exists.
	fn exists(&self, address: ObjectId) -> bool;
	/// Check whether an address has already been deleted.
	fn deleted(&self, address: ObjectId) -> bool;

	/// Set storage value of address at index.
	fn set_storage(&mut self, address: ObjectId, index: H256, value: H256) -> Result<(), ExitError>;
	/// Create a log owned by address with given topics and data.
	fn log(&mut self, address: ObjectId, topics: Vec<H256>, data: Vec<u8>) -> Result<(), ExitError>;
	/// Mark an address to be deleted, with funds transferred to target.
	fn mark_delete(&mut self, address: ObjectId, target: ObjectId) -> Result<(), ExitError>;
	/// Invoke a create operation.
	fn create(
		&mut self,
		caller: ObjectId,
		scheme: CreateScheme,
		value: u64,
		init_code: Vec<u8>,
		target_gas: Option<u64>,
	) -> Capture<(ExitReason, Option<ObjectId>, Vec<u8>), Self::CreateInterrupt>;
	/// Feed in create feedback.
	fn create_feedback(
		&mut self,
		_feedback: Self::CreateFeedback
	) -> Result<(), ExitError> {
		Ok(())
	}
	/// Invoke a call operation.
	fn call(
		&mut self,
		code_address: ObjectId,
		transfer: Option<Transfer>,
		input: Vec<u8>,
		target_gas: Option<u64>,
		is_static: bool,
		context: Context,
	) -> Capture<(ExitReason, Vec<u8>), Self::CallInterrupt>;
	/// Feed in call feedback.
	fn call_feedback(
		&mut self,
		_feedback: Self::CallFeedback
	) -> Result<(), ExitError> {
		Ok(())
	}

	/// Pre-validation step for the runtime.
	fn pre_validate(
		&mut self,
		context: &Context,
		opcode: Opcode,
		stack: &Stack
	) -> Result<(), ExitError>;
	/// Handle other unknown external opcodes.
	fn other(
		&mut self,
		_opcode: Opcode,
		_stack: &mut Machine
	) -> Result<(), ExitError> {
		Err(ExitError::OutOfGas)
	}
}
