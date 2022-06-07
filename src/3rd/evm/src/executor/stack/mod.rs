mod state;

pub use self::state::{MemoryStackSubstate, MemoryStackState, StackState};

use core::{convert::Infallible, cmp::min};
use alloc::{rc::Rc, vec::Vec};
use primitive_types::{U256, H256};
use sha3::{Keccak256, Digest};
use crate::{Stack, Opcode, Capture, Handler, Transfer,
			Context, CreateScheme, Runtime, Config};
use crate::gasometer::{self, Gasometer};
use cyfs_base::{ObjectId, ContractId};
use cyfs_base_meta::evm_def::{ExitError, ExitReason, ExitSucceed};

pub enum StackExitKind {
	Succeeded,
	Reverted,
	Failed,
}

pub struct StackSubstateMetadata<'config> {
	gasometer: Gasometer<'config>,
	is_static: bool,
	depth: Option<usize>,
}

impl<'config> StackSubstateMetadata<'config> {
	pub fn new(
		gas_limit: u64,
		config: &'config Config,
	) -> Self {
		Self {
			gasometer: Gasometer::new(gas_limit, config),
			is_static: false,
			depth: None,
		}
	}

	pub fn swallow_commit(&mut self, other: Self) -> Result<(), ExitError> {
		self.gasometer.record_stipend(other.gasometer.gas())?;
		self.gasometer.record_refund(other.gasometer.refunded_gas())?;

		Ok(())
	}

	pub fn swallow_revert(&mut self, other: Self) -> Result<(), ExitError> {
		self.gasometer.record_stipend(other.gasometer.gas())?;

		Ok(())
	}

	pub fn swallow_discard(&mut self, _other: Self) -> Result<(), ExitError> {
		Ok(())
	}

	pub fn spit_child(&self, gas_limit: u64, is_static: bool) -> Self {
		Self {
			gasometer: Gasometer::new(gas_limit, self.gasometer.config()),
			is_static: is_static || self.is_static,
			depth: match self.depth {
				None => Some(0),
				Some(n) => Some(n + 1),
			},
		}
	}

	pub fn gasometer(&self) -> &Gasometer<'config> {
		&self.gasometer
	}

	pub fn gasometer_mut(&mut self) -> &mut Gasometer<'config> {
		&mut self.gasometer
	}

	pub fn is_static(&self) -> bool {
		self.is_static
	}

	pub fn depth(&self) -> Option<usize> {
		self.depth
	}
}

/// Stack-based executor.
pub struct StackExecutor<'config, S> {
	config: &'config Config,
	precompile: fn(ObjectId, &[u8], Option<u64>, &Context) -> Option<Result<(ExitSucceed, Vec<u8>, u64), ExitError>>,
	state: S,
}

fn no_precompile(
	_address: ObjectId,
	_input: &[u8],
	_target_gas: Option<u64>,
	_context: &Context,
) -> Option<Result<(ExitSucceed, Vec<u8>, u64), ExitError>> {
	None
}

impl<'config, S: StackState<'config>> StackExecutor<'config, S> {
	/// Create a new stack-based executor.
	pub fn new(
		state: S,
		config: &'config Config,
	) -> Self {
		Self::new_with_precompile(state, config, no_precompile)
	}

	/// Return a reference of the Config.
	pub fn config(
		&self
	) -> &'config Config {
		self.config
	}

	/// Create a new stack-based executor with given precompiles.
	pub fn new_with_precompile(
		state: S,
		config: &'config Config,
		precompile: fn(ObjectId, &[u8], Option<u64>, &Context) -> Option<Result<(ExitSucceed, Vec<u8>, u64), ExitError>>,
	) -> Self {
		Self {
			config,
			precompile,
			state,
		}
	}

	pub fn state(&self) -> &S {
		&self.state
	}

	pub fn state_mut(&mut self) -> &mut S {
		&mut self.state
	}

	pub fn into_state(self) -> S {
		self.state
	}

	/// Create a substate executor from the current executor.
	pub fn enter_substate(
		&mut self,
		gas_limit: u64,
		is_static: bool,
	) {
		self.state.enter(gas_limit, is_static);
	}

	/// Exit a substate. Panic if it results an empty substate stack.
	pub fn exit_substate(
		&mut self,
		kind: StackExitKind,
	) -> Result<(), ExitError> {
		match kind {
			StackExitKind::Succeeded => self.state.exit_commit(),
			StackExitKind::Reverted => self.state.exit_revert(),
			StackExitKind::Failed => self.state.exit_discard(),
		}
	}

	/// Execute the runtime until it returns.
	pub fn execute(&mut self, runtime: &mut Runtime) -> ExitReason {
		match runtime.run(self) {
			Capture::Exit(s) => s,
			Capture::Trap(_) => unreachable!("Trap is Infallible"),
		}
	}

	/// Get remaining gas.
	pub fn gas(&self) -> u64 {
		self.state.metadata().gasometer.gas()
	}

	/// Execute a `CREATE` transaction.
	pub fn transact_create(
		&mut self,
		caller: ObjectId,
		value: u64,
		init_code: Vec<u8>,
		gas_limit: u64,
	) -> (ExitReason, Option<ObjectId>, Vec<u8>) {
		let transaction_cost = gasometer::create_transaction_cost(&init_code);
		match self.state.metadata_mut().gasometer.record_transaction(transaction_cost) {
			Ok(()) => (),
			Err(e) => return (e.into(), None, Vec::new()),
		}

		match self.create_inner(
			caller,
			CreateScheme::Legacy { caller },
			value,
			init_code,
			Some(gas_limit),
			false,
		) {
			Capture::Exit(ret) => ret,
			Capture::Trap(_) => unreachable!(),
		}
	}

	/// Execute a `CREATE2` transaction.
	pub fn transact_create2(
		&mut self,
		caller: ObjectId,
		value: u64,
		init_code: Vec<u8>,
		salt: H256,
		gas_limit: u64,
	) -> (ExitReason, Option<ObjectId>, Vec<u8>) {
		let transaction_cost = gasometer::create_transaction_cost(&init_code);
		match self.state.metadata_mut().gasometer.record_transaction(transaction_cost) {
			Ok(()) => (),
			Err(e) => return (e.into(), None, Vec::new()),
		}
		let code_hash = H256::from_slice(Keccak256::digest(&init_code).as_slice());

		match self.create_inner(
			caller,
			CreateScheme::Create2 { caller, code_hash, salt },
			value,
			init_code,
			Some(gas_limit),
			false,
		) {
			Capture::Exit(ret) => ret,
			Capture::Trap(_) => unreachable!(),
		}
	}

	/// Execute a `CALL` transaction.
	pub fn transact_call(
		&mut self,
		caller: ObjectId,
		address: ObjectId,
		value: u64,
		data: Vec<u8>,
		gas_limit: u64,
	) -> (ExitReason, Vec<u8>) {
		let transaction_cost = gasometer::call_transaction_cost(&data);
		match self.state.metadata_mut().gasometer.record_transaction(transaction_cost) {
			Ok(()) => (),
			Err(e) => return (e.into(), Vec::new()),
		}

		let context = Context {
			caller,
			address,
			apparent_value: value,
		};
		match self.call_inner(address, Some(Transfer {
			source: caller,
			target: address,
			value
		}), data, Some(gas_limit), false, false, false, context) {
			Capture::Exit((s, v)) => (s, v),
			Capture::Trap(_) => unreachable!(),
		}
	}

	/// Get used gas for the current executor, given the price.
	pub fn used_gas(
		&self,
	) -> u64 {
		self.state.metadata().gasometer.total_used_gas() -
			min(self.state.metadata().gasometer.total_used_gas() / 2,
				self.state.metadata().gasometer.refunded_gas() as u64)
	}

	/// Get fee needed for the current executor, given the price.
	pub fn fee(
		&self,
		price: U256,
	) -> U256 {
		let used_gas = self.used_gas();
		U256::from(used_gas) * price
	}

	/// Get account nonce.
	pub fn nonce(&self, address: ObjectId) -> u64 {
		self.state.basic(address).nonce
	}

	/// Get the create address from given scheme.
	pub fn create_address(&self, scheme: CreateScheme) -> ObjectId {
		match scheme {
			CreateScheme::Create2 { caller, code_hash, salt } => {
				let mut hasher = Keccak256::new();
				hasher.input(&[0xff]);
				hasher.input(caller.as_slice());
				hasher.input(&salt[..]);
				hasher.input(&code_hash[..]);
				ContractId::from_hash_256(hasher.result()).into()
			},
			CreateScheme::Legacy { caller } => {
				let nonce = self.nonce(caller);
				let mut stream = rlp::RlpStream::new_list(2);
				let h256_caller:H256 = caller.into();
				stream.append(&h256_caller);
				stream.append(&nonce);
				ContractId::from_hash_256(Keccak256::digest(&stream.out())).into()
			},
			CreateScheme::Fixed(naddress) => {
				naddress
			},
		}
	}

	fn create_inner(
		&mut self,
		caller: ObjectId,
		scheme: CreateScheme,
		value: u64,
		init_code: Vec<u8>,
		target_gas: Option<u64>,
		take_l64: bool,
	) -> Capture<(ExitReason, Option<ObjectId>, Vec<u8>), Infallible> {
		macro_rules! try_or_fail {
			( $e:expr ) => {
				match $e {
					Ok(v) => v,
					Err(e) => return Capture::Exit((e.into(), None, Vec::new())),
				}
			}
		}

		fn l64(gas: u64) -> u64 {
			gas - gas / 64
		}

		if let Some(depth) = self.state.metadata().depth {
			if depth > self.config.call_stack_limit {
				return Capture::Exit((ExitError::CallTooDeep.into(), None, Vec::new()))
			}
		}

		if self.balance(caller) < value {
			return Capture::Exit((ExitError::OutOfFund.into(), None, Vec::new()))
		}

		let after_gas = if take_l64 && self.config.call_l64_after_gas {
			if self.config.estimate {
				let initial_after_gas = self.state.metadata().gasometer.gas();
				let diff = initial_after_gas - l64(initial_after_gas);
				try_or_fail!(self.state.metadata_mut().gasometer.record_cost(diff));
				self.state.metadata().gasometer.gas()
			} else {
				l64(self.state.metadata().gasometer.gas())
			}
		} else {
			self.state.metadata().gasometer.gas()
		};

		let target_gas = target_gas.unwrap_or(after_gas);

		let gas_limit = min(after_gas, target_gas);
		try_or_fail!(
			self.state.metadata_mut().gasometer.record_cost(gas_limit)
		);

		let address = self.create_address(scheme);

		self.enter_substate(gas_limit, false);

		{
			if self.code_size(address) != U256::zero() {
				let _ = self.exit_substate(StackExitKind::Failed);
				return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()))
			}

			if self.nonce(address) > 0 {
				let _ = self.exit_substate(StackExitKind::Failed);
				return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()))
			}

			self.state.reset_storage(address);
		}

		let context = Context {
			address,
			caller,
			apparent_value: value,
		};
		let transfer = Transfer {
			source: caller,
			target: address,
			value,
		};
		match self.state.transfer(transfer) {
			Ok(()) => (),
			Err(e) => {
				let _ = self.exit_substate(StackExitKind::Reverted);
				return Capture::Exit((ExitReason::Error(e), None, Vec::new()))
			},
		}

		if self.config.create_increase_nonce {
			self.state.inc_nonce(address);
		}

		let mut runtime = Runtime::new(
			Rc::new(init_code),
			Rc::new(Vec::new()),
			context,
			self.config,
		);

		let reason = self.execute(&mut runtime);
		log::debug!(target: "evm", "Create execution using address {}: {:?}", address, reason);

		match reason {
			ExitReason::Succeed(s) => {
				let out = runtime.machine().return_value();

				if let Some(limit) = self.config.create_contract_limit {
					if out.len() > limit {
						self.state.metadata_mut().gasometer.fail();
						let _ = self.exit_substate(StackExitKind::Failed);
						return Capture::Exit((ExitError::CreateContractLimit.into(), None, Vec::new()))
					}
				}

				match self.state.metadata_mut().gasometer.record_deposit(out.len()) {
					Ok(()) => {
						let e = self.exit_substate(StackExitKind::Succeeded);
						self.state.set_code(address, out);
						try_or_fail!(e);
						Capture::Exit((ExitReason::Succeed(s), Some(address), Vec::new()))
					},
					Err(e) => {
						let _ = self.exit_substate(StackExitKind::Failed);
						Capture::Exit((ExitReason::Error(e), None, Vec::new()))
					},
				}
			},
			ExitReason::Error(e) => {
				self.state.metadata_mut().gasometer.fail();
				let _ = self.exit_substate(StackExitKind::Failed);
				Capture::Exit((ExitReason::Error(e), None, Vec::new()))
			},
			ExitReason::Revert(e) => {
				let _ = self.exit_substate(StackExitKind::Reverted);
				Capture::Exit((ExitReason::Revert(e), None, runtime.machine().return_value()))
			},
			ExitReason::Fatal(e) => {
				self.state.metadata_mut().gasometer.fail();
				let _ = self.exit_substate(StackExitKind::Failed);
				Capture::Exit((ExitReason::Fatal(e), None, Vec::new()))
			},
		}
	}

	fn call_inner(
		&mut self,
		code_address: ObjectId,
		transfer: Option<Transfer>,
		input: Vec<u8>,
		target_gas: Option<u64>,
		is_static: bool,
		take_l64: bool,
		take_stipend: bool,
		context: Context,
	) -> Capture<(ExitReason, Vec<u8>), Infallible> {
		macro_rules! try_or_fail {
			( $e:expr ) => {
				match $e {
					Ok(v) => v,
					Err(e) => return Capture::Exit((e.into(), Vec::new())),
				}
			}
		}

		fn l64(gas: u64) -> u64 {
			gas - gas / 64
		}

		let after_gas = if take_l64 && self.config.call_l64_after_gas {
			if self.config.estimate {
				let initial_after_gas = self.state.metadata().gasometer.gas();
				let diff = initial_after_gas - l64(initial_after_gas);
				try_or_fail!(self.state.metadata_mut().gasometer.record_cost(diff));
				self.state.metadata().gasometer.gas()
			} else {
				l64(self.state.metadata().gasometer.gas())
			}
		} else {
			self.state.metadata().gasometer.gas()
		};

		let target_gas = target_gas.unwrap_or(after_gas);
		let mut gas_limit = min(target_gas, after_gas);

		try_or_fail!(
			self.state.metadata_mut().gasometer.record_cost(gas_limit)
		);

		if let Some(transfer) = transfer.as_ref() {
			if take_stipend && transfer.value != 0 {
				gas_limit = gas_limit.saturating_add(self.config.call_stipend);
			}
		}

		let code = self.code(code_address);

		self.enter_substate(gas_limit, is_static);
		self.state.touch(context.address);

		if let Some(depth) = self.state.metadata().depth {
			if depth > self.config.call_stack_limit {
				let _ = self.exit_substate(StackExitKind::Reverted);
				return Capture::Exit((ExitError::CallTooDeep.into(), Vec::new()))
			}
		}

		if let Some(transfer) = transfer {
			match self.state.transfer(transfer) {
				Ok(()) => (),
				Err(e) => {
					let _ = self.exit_substate(StackExitKind::Reverted);
					return Capture::Exit((ExitReason::Error(e), Vec::new()))
				},
			}
		}

		if let Some(ret) = (self.precompile)(code_address, &input, Some(gas_limit), &context) {
			return match ret {
				Ok((s, out, cost)) => {
					let _ = self.state.metadata_mut().gasometer.record_cost(cost);
					let _ = self.exit_substate(StackExitKind::Succeeded);
					Capture::Exit((ExitReason::Succeed(s), out))
				},
				Err(e) => {
					let _ = self.exit_substate(StackExitKind::Failed);
					Capture::Exit((ExitReason::Error(e), Vec::new()))
				},
			}
		}

		let mut runtime = Runtime::new(
			Rc::new(code),
			Rc::new(input),
			context,
			self.config,
		);
		let reason = self.execute(&mut runtime);
		log::debug!(target: "evm", "Call execution using address {}: {:?}", code_address, reason);

		match reason {
			ExitReason::Succeed(s) => {
				let _ = self.exit_substate(StackExitKind::Succeeded);
				Capture::Exit((ExitReason::Succeed(s), runtime.machine().return_value()))
			},
			ExitReason::Error(e) => {
				let _ = self.exit_substate(StackExitKind::Failed);
				Capture::Exit((ExitReason::Error(e), Vec::new()))
			},
			ExitReason::Revert(e) => {
				let _ = self.exit_substate(StackExitKind::Reverted);
				Capture::Exit((ExitReason::Revert(e), runtime.machine().return_value()))
			},
			ExitReason::Fatal(e) => {
				self.state.metadata_mut().gasometer.fail();
				let _ = self.exit_substate(StackExitKind::Failed);
				Capture::Exit((ExitReason::Fatal(e), Vec::new()))
			},
		}
	}
}

impl<'config, S: StackState<'config>> Handler for StackExecutor<'config, S> {
	type CreateInterrupt = Infallible;
	type CreateFeedback = Infallible;
	type CallInterrupt = Infallible;
	type CallFeedback = Infallible;

	fn balance(&self, address: ObjectId) -> u64 {
		self.state.basic(address).balance
	}

	fn code_size(&self, address: ObjectId) -> U256 {
		U256::from(self.state.code(address).len())
	}

	fn code_hash(&self, address: ObjectId) -> H256 {
		if !self.exists(address) {
			return H256::default()
		}

		H256::from_slice(Keccak256::digest(&self.state.code(address)).as_slice())
	}

	fn code(&self, address: ObjectId) -> Vec<u8> {
		self.state.code(address)
	}

	fn storage(&self, address: ObjectId, index: H256) -> H256 {
		self.state.storage(address, index)
	}

	fn original_storage(&self, address: ObjectId, index: H256) -> H256 {
		self.state.original_storage(address, index).unwrap_or_default()
	}

	fn exists(&self, address: ObjectId) -> bool {
		if self.config.empty_considered_exists {
			self.state.exists(address)
		} else {
			self.state.exists(address) && !self.state.is_empty(address)
		}
	}

	fn gas_left(&self) -> U256 {
		U256::from(self.state.metadata().gasometer.gas())
	}

	fn gas_price(&self) -> U256 { self.state.gas_price() }
	fn origin(&self) -> ObjectId { self.state.origin() }
	fn block_coinbase(&self) -> ObjectId {
		self.state.block_coinbase()
	}
	fn block_hash(&self, number: U256) -> H256 { self.state.block_hash(number) }
	fn block_number(&self) -> U256 { self.state.block_number() }
	fn block_timestamp(&self) -> U256 { self.state.block_timestamp() }
	fn block_gas_limit(&self) -> U256 { self.state.block_gas_limit() }
	fn chain_id(&self) -> U256 { self.state.chain_id() }

	fn deleted(&self, address: ObjectId) -> bool {
		self.state.deleted(address)
	}

	fn set_storage(&mut self, address: ObjectId, index: H256, value: H256) -> Result<(), ExitError> {
		self.state.set_storage(address, index, value);
		Ok(())
	}

	fn log(&mut self, address: ObjectId, topics: Vec<H256>, data: Vec<u8>) -> Result<(), ExitError> {
		self.state.log(address, topics, data);
		Ok(())
	}

	fn mark_delete(&mut self, address: ObjectId, target: ObjectId) -> Result<(), ExitError> {
		let balance = self.balance(address);

		self.state.transfer(Transfer {
			source: address,
			target: target,
			value: balance,
		})?;
		self.state.reset_balance(address);
		self.state.set_deleted(address);

		Ok(())
	}

	fn create(
		&mut self,
		caller: ObjectId,
		scheme: CreateScheme,
		value: u64,
		init_code: Vec<u8>,
		target_gas: Option<u64>,
	) -> Capture<(ExitReason, Option<ObjectId>, Vec<u8>), Self::CreateInterrupt> {
		self.create_inner(caller, scheme, value, init_code, target_gas, true)
	}

	fn call(
		&mut self,
		code_address: ObjectId,
		transfer: Option<Transfer>,
		input: Vec<u8>,
		target_gas: Option<u64>,
		is_static: bool,
		context: Context,
	) -> Capture<(ExitReason, Vec<u8>), Self::CallInterrupt> {
		self.call_inner(code_address, transfer, input, target_gas, is_static, true, true, context)
	}

	#[inline]
	fn pre_validate(
		&mut self,
		context: &Context,
		opcode: Opcode,
		stack: &Stack
	) -> Result<(), ExitError> {
		// println!("Running opcode: {:x}, Pre gas-left: {:?}", opcode.0, self.gas());

		if let Some(cost) = gasometer::static_opcode_cost(opcode) {
			self.state.metadata_mut().gasometer.record_cost(cost)?;
		} else {
			let is_static = self.state.metadata().is_static;
			let (gas_cost, memory_cost) = gasometer::dynamic_opcode_cost(
				context.address, opcode, stack, is_static, &self.config, self
			)?;

			let gasometer = &mut self.state.metadata_mut().gasometer;

			gasometer.record_dynamic_cost(gas_cost, memory_cost)?;
		}

		Ok(())
	}
}
