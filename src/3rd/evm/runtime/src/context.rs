use primitive_types::{H256};
use cyfs_base::ObjectId;

/// Create scheme.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum CreateScheme {
	/// Legacy create scheme of `CREATE`.
	Legacy {
		/// Caller of the create.
		caller: ObjectId,
	},
	/// Create scheme of `CREATE2`.
	Create2 {
		/// Caller of the create.
		caller: ObjectId,
		/// Code hash.
		code_hash: H256,
		/// Salt.
		salt: H256,
	},
	/// Create at a fixed location.
	Fixed(ObjectId),
}

/// Call scheme.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum CallScheme {
	/// `CALL`
	Call,
	/// `CALLCODE`
	CallCode,
	/// `DELEGATECALL`
	DelegateCall,
	/// `STATICCALL`
	StaticCall,
}

/// Context of the runtime.
#[derive(Clone, Debug)]
pub struct Context {
	/// Execution address.
	pub address: ObjectId,
	/// Caller of the EVM.
	pub caller: ObjectId,
	/// Apparent value of the EVM.
	pub apparent_value: u64,
}
