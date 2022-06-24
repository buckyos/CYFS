use primitive_types::H256;
use alloc::vec::Vec;
use cyfs_base_meta::evm_def::ExitError;

/// EVM stack.
#[derive(Clone, Debug)]
pub struct Stack {
	data: Vec<H256>,
	limit: usize,
}

impl Stack {
	/// Create a new stack with given limit.
	pub fn new(limit: usize) -> Self {
		Self {
			data: Vec::new(),
			limit,
		}
	}

	#[inline]
	/// Stack limit.
	pub fn limit(&self) -> usize {
		self.limit
	}

	#[inline]
	/// Stack length.
	pub fn len(&self) -> usize {
		self.data.len()
	}

	#[inline]
	/// Stack data.
	pub fn data(&self) -> &Vec<H256> {
		&self.data
	}

	#[inline]
	/// Pop a value from the stack. If the stack is already empty, returns the
	/// `StackUnderflow` error.
	pub fn pop(&mut self) -> Result<H256, ExitError> {
		let ret =self.data.pop().ok_or(ExitError::StackUnderflow)?;
		// println!("stack pop value {}", hex::encode(&ret));
		Ok(ret)
	}

	#[inline]
	/// Push a new value into the stack. If it will exceed the stack limit,
	/// returns `StackOverflow` error and leaves the stack unchanged.
	pub fn push(&mut self, value: H256) -> Result<(), ExitError> {
		// println!("stack push value {}", hex::encode(&value));
		if self.data.len() + 1 > self.limit {
			return Err(ExitError::StackOverflow)
		}
		self.data.push(value);
		Ok(())
	}

	#[inline]
	/// Peek a value at given index for the stack, where the top of
	/// the stack is at index `0`. If the index is too large,
	/// `StackError::Underflow` is returned.
	pub fn peek(&self, no_from_top: usize) -> Result<H256, ExitError> {
		if self.data.len() > no_from_top {
			let ret = self.data[self.data.len() - no_from_top - 1];
			// println!("stack peek value {} at {}", hex::encode(&ret), no_from_top);
			Ok(ret)
		} else {
			Err(ExitError::StackUnderflow)
		}
	}

	#[inline]
	/// Set a value at given index for the stack, where the top of the
	/// stack is at index `0`. If the index is too large,
	/// `StackError::Underflow` is returned.
	pub fn set(&mut self, no_from_top: usize, val: H256) -> Result<(), ExitError> {
		if self.data.len() > no_from_top {
			// println!("stack set value {} at {}", hex::encode(&val), no_from_top);
			let len = self.data.len();
			self.data[len - no_from_top - 1] = val;
			Ok(())
		} else {
			Err(ExitError::StackUnderflow)
		}
	}
}
