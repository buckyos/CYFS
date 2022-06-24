use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use primitive_types::{H256, U256};
use super::{Basic, Backend, ApplyBackend, Apply, Log};
use cyfs_base::ObjectId;

/// Vivinity value of a memory backend.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "with-codec", derive(codec::Encode, codec::Decode))]
#[cfg_attr(feature = "with-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemoryVicinity {
	/// Gas price.
	pub gas_price: U256,
	/// Origin.
	pub origin: ObjectId,
	/// Chain ID.
	pub chain_id: U256,
	pub coinbase: ObjectId,
	/// Environmental block hashes.
	pub block_hashes: Vec<H256>,
	/// Environmental block number.
	pub block_number: U256,
	/// Environmental block timestamp.
	pub block_timestamp: U256,
	/// Environmental block gas limit.
	pub block_gas_limit: U256,
}

/// Account information of a memory backend.
#[derive(Default, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "with-codec", derive(codec::Encode, codec::Decode))]
#[cfg_attr(feature = "with-serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemoryAccount {
	/// Account nonce.
	pub nonce: u64,
	/// Account balance.
	pub balance: u64,
	/// Full account storage.
	pub storage: BTreeMap<H256, H256>,
	/// Account code.
	pub code: Vec<u8>,
}

/// Memory backend, storing all state values in a `BTreeMap` in memory.
#[derive(Clone, Debug)]
pub struct MemoryBackend<'vicinity> {
	vicinity: &'vicinity MemoryVicinity,
	state: BTreeMap<ObjectId, MemoryAccount>,
	logs: Vec<Log>,
}

impl<'vicinity> MemoryBackend<'vicinity> {
	/// Create a new memory backend.
	pub fn new(vicinity: &'vicinity MemoryVicinity, state: BTreeMap<ObjectId, MemoryAccount>) -> Self {
		Self {
			vicinity,
			state,
			logs: Vec::new(),
		}
	}

	/// Get the underlying `BTreeMap` storing the state.
	pub fn state(&self) -> &BTreeMap<ObjectId, MemoryAccount> {
		&self.state
	}

	pub fn logs(&self) -> &Vec<Log> {&self.logs}
}

impl<'vicinity> Backend for MemoryBackend<'vicinity> {
	fn gas_price(&self) -> U256 { self.vicinity.gas_price }
	fn origin(&self) -> ObjectId { self.vicinity.origin }

	fn block_coinbase(&self) -> ObjectId {
		self.vicinity.coinbase
	}

	fn block_hash(&self, number: U256) -> H256 {
		if number >= self.vicinity.block_number ||
			self.vicinity.block_number - number - U256::one() >= U256::from(self.vicinity.block_hashes.len())
		{
			H256::default()
		} else {
			let index = (self.vicinity.block_number - number - U256::one()).as_usize();
			self.vicinity.block_hashes[index]
		}
	}
	fn block_number(&self) -> U256 { self.vicinity.block_number }
	fn block_timestamp(&self) -> U256 { self.vicinity.block_timestamp }
	fn block_gas_limit(&self) -> U256 { self.vicinity.block_gas_limit }

	fn chain_id(&self) -> U256 { self.vicinity.chain_id }

	fn exists(&self, address: ObjectId) -> bool {
		self.state.contains_key(&address)
	}

	fn basic(&self, address: ObjectId) -> Basic {
		self.state.get(&address).map(|a| {
			Basic { balance: a.balance, nonce: a.nonce }
		}).unwrap_or_default()
	}

	fn code(&self, address: ObjectId) -> Vec<u8> {
		self.state.get(&address).map(|v| v.code.clone()).unwrap_or_default()
	}

	fn storage(&self, address: ObjectId, index: H256) -> H256 {
		self.state.get(&address)
			.map(|v| v.storage.get(&index).cloned().unwrap_or(H256::default()))
			.unwrap_or(H256::default())
	}

	fn original_storage(&self, address: ObjectId, index: H256) -> Option<H256> {
		Some(self.storage(address, index))
	}
}

impl<'vicinity> ApplyBackend for MemoryBackend<'vicinity> {
	fn apply<A, I, L>(
		&mut self,
		values: A,
		logs: L,
		delete_empty: bool,
	) where
		A: IntoIterator<Item=Apply<I>>,
		I: IntoIterator<Item=(H256, H256)>,
		L: IntoIterator<Item=Log>,
	{
		for apply in values {
			match apply {
				Apply::Modify {
					address, basic, code, storage, reset_storage,
				} => {
					let is_empty = {
						let account = self.state.entry(address).or_insert(Default::default());
						account.balance = basic.balance;
						account.nonce = basic.nonce;
						if let Some(code) = code {
							account.code = code;
						}

						if reset_storage {
							account.storage = BTreeMap::new();
						}

						let zeros = account.storage.iter()
							.filter(|(_, v)| v == &&H256::default())
							.map(|(k, _)| k.clone())
							.collect::<Vec<H256>>();

						for zero in zeros {
							account.storage.remove(&zero);
						}

						for (index, value) in storage {
							if value == H256::default() {
								account.storage.remove(&index);
							} else {
								account.storage.insert(index, value);
							}
						}

						account.balance == 0 &&
							account.nonce == 0 &&
							account.code.len() == 0
					};

					if is_empty && delete_empty {
						self.state.remove(&address);
					}
				},
				Apply::Delete {
					address,
				} => {
					self.state.remove(&address);
				},
			}
		}

		for log in logs {
			self.logs.push(log);
		}
	}
}
