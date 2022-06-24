use core::mem;
use alloc::{vec::Vec, boxed::Box, collections::{BTreeMap, BTreeSet}};
use primitive_types::{H256, U256};
use crate::{ExitError, Transfer};
use crate::backend::{Basic, Log, Backend, Apply};
use crate::executor::stack::StackSubstateMetadata;
use cyfs_base::ObjectId;

#[derive(Clone, Debug)]
struct MemoryStackAccount {
	pub basic: Basic,
	pub code: Option<Vec<u8>>,
	pub reset: bool,
}

pub struct MemoryStackSubstate<'config> {
	metadata: StackSubstateMetadata<'config>,
	parent: Option<Box<MemoryStackSubstate<'config>>>,
	logs: Vec<Log>,
	accounts: BTreeMap<ObjectId, MemoryStackAccount>,
	storages: BTreeMap<(ObjectId, H256), H256>,
	deletes: BTreeSet<ObjectId>,
}

impl<'config> MemoryStackSubstate<'config> {
	pub fn new(metadata: StackSubstateMetadata<'config>) -> Self {
		Self {
			metadata,
			parent: None,
			logs: Vec::new(),
			accounts: BTreeMap::new(),
			storages: BTreeMap::new(),
			deletes: BTreeSet::new(),
		}
	}

	pub fn metadata(&self) -> &StackSubstateMetadata<'config> {
		&self.metadata
	}

	pub fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
		&mut self.metadata
	}

	/// Deconstruct the executor, return state to be applied. Panic if the
	/// executor is not in the top-level substate.
	#[must_use]
	pub fn deconstruct<B: Backend>(
		mut self, backend: &B,
	) -> (impl IntoIterator<Item=Apply<impl IntoIterator<Item=(H256, H256)>>>,
 		  impl IntoIterator<Item=Log>)
	{
		assert!(self.parent.is_none());

		let mut applies = Vec::<Apply<BTreeMap<H256, H256>>>::new();

		let mut addresses = BTreeSet::new();

		for address in self.accounts.keys() {
			addresses.insert(*address);
		}

		for (address, _) in self.storages.keys() {
			addresses.insert(*address);
		}

		for address in addresses {
			if self.deletes.contains(&address) {
				continue
			}

			let mut storage = BTreeMap::new();
			for ((oa, ok), ov) in &self.storages {
				if *oa == address {
					storage.insert(*ok, *ov);
				}
			}

			let apply = {
				let account = self.account_mut(address, backend);

				Apply::Modify {
					address,
					basic: account.basic.clone(),
					code: account.code.clone(),
					storage,
					reset_storage: account.reset,
				}
			};

			applies.push(apply);
		}

		for address in self.deletes {
			applies.push(Apply::Delete { address });
		}

		(applies, self.logs)
	}

	pub fn enter(&mut self, gas_limit: u64, is_static: bool) {
		let mut entering = Self {
			metadata: self.metadata.spit_child(gas_limit, is_static),
			parent: None,
			logs: Vec::new(),
			accounts: BTreeMap::new(),
			storages: BTreeMap::new(),
			deletes: BTreeSet::new(),
		};
		mem::swap(&mut entering, self);

		self.parent = Some(Box::new(entering));
	}

	pub fn exit_commit(&mut self) -> Result<(), ExitError> {
		let mut exited = *self.parent.take().expect("Cannot commit on root substate");
		mem::swap(&mut exited, self);

		self.metadata.swallow_commit(exited.metadata)?;
		self.logs.append(&mut exited.logs);

		let mut resets = BTreeSet::new();
		for (address, account) in &exited.accounts {
			if account.reset {
				resets.insert(*address);
			}
		}
		let mut reset_keys = BTreeSet::new();
		for (address, key) in self.storages.keys() {
			if resets.contains(&address) {
				reset_keys.insert((*address, *key));
			}
		}
		for (address, key) in reset_keys {
			self.storages.remove(&(address, key));
		}

		self.accounts.append(&mut exited.accounts);
		self.storages.append(&mut exited.storages);
		self.deletes.append(&mut exited.deletes);

		Ok(())
	}

	pub fn exit_revert(&mut self) -> Result<(), ExitError> {
		let mut exited = *self.parent.take().expect("Cannot discard on root substate");
		mem::swap(&mut exited, self);

		self.metadata.swallow_revert(exited.metadata)?;

		Ok(())
	}

	pub fn exit_discard(&mut self) -> Result<(), ExitError> {
		let mut exited = *self.parent.take().expect("Cannot discard on root substate");
		mem::swap(&mut exited, self);

		self.metadata.swallow_discard(exited.metadata)?;

		Ok(())
	}

	fn known_account(&self, address: ObjectId) -> Option<&MemoryStackAccount> {
		if let Some(account) = self.accounts.get(&address) {
			Some(account)
		} else if let Some(parent) = self.parent.as_ref() {
			parent.known_account(address)
		} else {
			None
		}
	}

	pub fn known_basic(&self, address: ObjectId) -> Option<Basic> {
		self.known_account(address).map(|acc| acc.basic.clone())
	}

	pub fn known_code(&self, address: ObjectId) -> Option<Vec<u8>> {
		self.known_account(address).and_then(|acc| acc.code.clone())
	}

	pub fn known_empty(&self, address: ObjectId) -> Option<bool> {
		if let Some(account) = self.known_account(address) {
			if account.basic.balance != 0 {
				return Some(false)
			}

			if account.basic.nonce != 0 {
				return Some(false)
			}

			if let Some(code) = &account.code {
				return Some(
					account.basic.balance == 0 &&
						account.basic.nonce == 0 &&
						code.len() == 0
				)
			}
		}

		None
	}

	pub fn known_storage(&self, address: ObjectId, key: H256) -> Option<H256> {
		if let Some(value) = self.storages.get(&(address, key)) {
			return Some(*value)
		}

		if let Some(account) = self.accounts.get(&address) {
			if account.reset {
				return Some(H256::default())
			}
		}

		if let Some(parent) = self.parent.as_ref() {
			return parent.known_storage(address, key)
		}

		None
	}

	pub fn known_original_storage(&self, address: ObjectId, key: H256) -> Option<H256> {
		if let Some(account) = self.accounts.get(&address) {
			if account.reset {
				return Some(H256::default())
			}
		}

		if let Some(parent) = self.parent.as_ref() {
			return parent.known_original_storage(address, key)
		}

		None
	}

	pub fn deleted(&self, address: ObjectId) -> bool {
		if self.deletes.contains(&address) {
			return true
		}

		if let Some(parent) = self.parent.as_ref() {
			return parent.deleted(address)
		}

		false
	}

	fn account_mut<B: Backend>(&mut self, address: ObjectId, backend: &B) -> &mut MemoryStackAccount {
		if !self.accounts.contains_key(&address) {
			let account = self.known_account(address)
				.cloned()
				.map(|mut v| {
					v.reset = false;
					v
				})
				.unwrap_or_else(|| MemoryStackAccount {
					basic: backend.basic(address),
					code: None,
					reset: false,
				});
			self.accounts.insert(address, account);
		}

		self.accounts.get_mut(&address).expect("New account was just inserted")
	}

	pub fn inc_nonce<B: Backend>(&mut self, address: ObjectId, backend: &B) {
		self.account_mut(address, backend).basic.nonce += 1;
	}

	pub fn set_storage(&mut self, address: ObjectId, key: H256, value: H256) {
		self.storages.insert((address, key), value);
	}

	pub fn reset_storage<B: Backend>(&mut self, address: ObjectId, backend: &B) {
		let mut removing = Vec::new();

		for (oa, ok) in self.storages.keys() {
			if *oa == address {
				removing.push(*ok);
			}
		}

		for ok in removing {
			self.storages.remove(&(address, ok));
		}

		self.account_mut(address, backend).reset = true;
	}

	pub fn log(&mut self, address: ObjectId, topics: Vec<H256>, data: Vec<u8>) {
		self.logs.push(Log {
			address, topics, data,
		});
	}

	pub fn set_deleted(&mut self, address: ObjectId) {
		self.deletes.insert(address);
	}

	pub fn set_code<B: Backend>(&mut self, address: ObjectId, code: Vec<u8>, backend: &B) {
		self.account_mut(address, backend).code = Some(code);
	}

	pub fn transfer<B: Backend>(&mut self, transfer: Transfer, backend: &B) -> Result<(), ExitError> {
		{
			let source = self.account_mut(transfer.source, backend);
			if source.basic.balance < transfer.value {
				return Err(ExitError::OutOfFund)
			}
			source.basic.balance -= transfer.value;
		}

		{
			let target = self.account_mut(transfer.target, backend);
			target.basic.balance = target.basic.balance.saturating_add(transfer.value);
		}

		Ok(())
	}

	// Only needed for jsontests.
	pub fn withdraw<B: Backend>(&mut self, address: ObjectId, value: u64, backend: &B) -> Result<(), ExitError> {
		let source = self.account_mut(address, backend);
		if source.basic.balance < value {
			return Err(ExitError::OutOfFund)
		}
		source.basic.balance -= value;

		Ok(())
	}

	// Only needed for jsontests.
	pub fn deposit<B: Backend>(&mut self, address: ObjectId, value: u64, backend: &B) {
		let target = self.account_mut(address, backend);
		target.basic.balance = target.basic.balance.saturating_add(value);
	}

	pub fn reset_balance<B: Backend>(&mut self, address: ObjectId, backend: &B) {
		self.account_mut(address, backend).basic.balance = 0;
	}

	pub fn touch<B: Backend>(&mut self, address: ObjectId, backend: &B) {
		self.account_mut(address, backend);
	}
}

pub trait StackState<'config>: Backend {
	fn metadata(&self) -> &StackSubstateMetadata<'config>;
	fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config>;

	fn enter(&mut self, gas_limit: u64, is_static: bool);
	fn exit_commit(&mut self) -> Result<(), ExitError>;
	fn exit_revert(&mut self) -> Result<(), ExitError>;
	fn exit_discard(&mut self) -> Result<(), ExitError>;

	fn is_empty(&self, address: ObjectId) -> bool;
	fn deleted(&self, address: ObjectId) -> bool;

	fn inc_nonce(&mut self, address: ObjectId);
	fn set_storage(&mut self, address: ObjectId, key: H256, value: H256);
	fn reset_storage(&mut self, address: ObjectId);
	fn log(&mut self, address: ObjectId, topics: Vec<H256>, data: Vec<u8>);
	fn set_deleted(&mut self, address: ObjectId);
	fn set_code(&mut self, address: ObjectId, code: Vec<u8>);
	fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError>;
	fn reset_balance(&mut self, address: ObjectId);
	fn touch(&mut self, address: ObjectId);
}

pub struct MemoryStackState<'backend, 'config, B> {
	backend: &'backend B,
	substate: MemoryStackSubstate<'config>,
}

impl<'backend, 'config, B: Backend> Backend for MemoryStackState<'backend, 'config, B> {
	fn gas_price(&self) -> U256 { self.backend.gas_price() }
	fn origin(&self) -> ObjectId { self.backend.origin() }

	fn block_coinbase(&self) -> ObjectId {
		self.backend.block_coinbase()
	}

	fn block_hash(&self, number: U256) -> H256 { self.backend.block_hash(number) }
	fn block_number(&self) -> U256 { self.backend.block_number() }
	fn block_timestamp(&self) -> U256 { self.backend.block_timestamp() }
	fn block_gas_limit(&self) -> U256 { self.backend.block_gas_limit() }
	fn chain_id(&self) -> U256 { self.backend.chain_id() }

	fn exists(&self, address: ObjectId) -> bool {
		self.substate.known_account(address).is_some() || self.backend.exists(address)
	}

	fn basic(&self, address: ObjectId) -> Basic {
		self.substate.known_basic(address).unwrap_or_else(|| self.backend.basic(address))
	}

	fn code(&self, address: ObjectId) -> Vec<u8> {
		self.substate.known_code(address).unwrap_or_else(|| self.backend.code(address))
	}

	fn storage(&self, address: ObjectId, key: H256) -> H256 {
		self.substate.known_storage(address, key)
			.unwrap_or_else(|| self.backend.storage(address, key))
	}

	fn original_storage(&self, address: ObjectId, key: H256) -> Option<H256> {
		if let Some(value) = self.substate.known_original_storage(address, key) {
			return Some(value)
		}

		self.backend.original_storage(address, key)
	}
}

impl<'backend, 'config, B: Backend> StackState<'config> for MemoryStackState<'backend, 'config, B> {
	fn metadata(&self) -> &StackSubstateMetadata<'config> {
		self.substate.metadata()
	}

	fn metadata_mut(&mut self) -> &mut StackSubstateMetadata<'config> {
		self.substate.metadata_mut()
	}

	fn enter(&mut self, gas_limit: u64, is_static: bool) {
		self.substate.enter(gas_limit, is_static)
	}

	fn exit_commit(&mut self) -> Result<(), ExitError> {
		self.substate.exit_commit()
	}

	fn exit_revert(&mut self) -> Result<(), ExitError> {
		self.substate.exit_revert()
	}

	fn exit_discard(&mut self) -> Result<(), ExitError> {
		self.substate.exit_discard()
	}

	fn is_empty(&self, address: ObjectId) -> bool {
		if let Some(known_empty) = self.substate.known_empty(address) {
			return known_empty
		}

		self.backend.basic(address).balance == 0 &&
			self.backend.basic(address).nonce == 0 &&
			self.backend.code(address).len() == 0
	}

	fn deleted(&self, address: ObjectId) -> bool {
		self.substate.deleted(address)
	}

	fn inc_nonce(&mut self, address: ObjectId) {
		self.substate.inc_nonce(address, self.backend);
	}

	fn set_storage(&mut self, address: ObjectId, key: H256, value: H256) {
		self.substate.set_storage(address, key, value)
	}

	fn reset_storage(&mut self, address: ObjectId) {
		self.substate.reset_storage(address, self.backend);
	}

	fn log(&mut self, address: ObjectId, topics: Vec<H256>, data: Vec<u8>) {
		self.substate.log(address, topics, data);
	}

	fn set_deleted(&mut self, address: ObjectId) {
		self.substate.set_deleted(address)
	}

	fn set_code(&mut self, address: ObjectId, code: Vec<u8>) {
		self.substate.set_code(address, code, self.backend)
	}

	fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
		self.substate.transfer(transfer, self.backend)
	}

	fn reset_balance(&mut self, address: ObjectId) {
		self.substate.reset_balance(address, self.backend)
	}

	fn touch(&mut self, address: ObjectId) {
		self.substate.touch(address, self.backend)
	}
}

impl<'backend, 'config, B: Backend> MemoryStackState<'backend, 'config, B> {
	pub fn new(metadata: StackSubstateMetadata<'config>, backend: &'backend B) -> Self {
		Self {
			backend,
			substate: MemoryStackSubstate::new(metadata),
		}
	}

	#[must_use]
	pub fn deconstruct(
		self
	) -> (impl IntoIterator<Item=Apply<impl IntoIterator<Item=(H256, H256)>>>,
 		  impl IntoIterator<Item=Log>)
	{
		self.substate.deconstruct(self.backend)
	}

	pub fn withdraw(&mut self, address: ObjectId, value: u64) -> Result<(), ExitError> {
		self.substate.withdraw(address, value, self.backend)
	}

	pub fn deposit(&mut self, address: ObjectId, value: u64) {
		self.substate.deposit(address, value, self.backend)
	}
}
