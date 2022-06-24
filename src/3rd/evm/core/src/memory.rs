use primitive_types::U256;
use core::cmp::{min, max};
use alloc::vec::Vec;
use cyfs_base_meta::evm_def::{ExitError, ExitFatal};

/// A sequencial memory. It uses Rust's `Vec` for internal
/// representation.
#[derive(Clone, Debug)]
pub struct Memory {
	data: Vec<u8>,
	effective_len: U256,
	limit: usize,
}

impl Memory {
	/// Create a new memory with the given limit.
	pub fn new(limit: usize) -> Self {
		Self {
			data: Vec::new(),
			effective_len: U256::zero(),
			limit,
		}
	}

	/// Memory limit.
	pub fn limit(&self) -> usize {
		self.limit
	}

	/// Get the length of the current memory range.
	pub fn len(&self) -> usize {
		self.data.len()
	}

	/// Get the effective length.
	pub fn effective_len(&self) -> U256 {
		self.effective_len
	}

	/// Return true if current effective memory range is zero.
	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	/// Return the full memory.
	pub fn data(&self) -> &Vec<u8> {
		&self.data
	}

	/// Resize the memory, making it cover the memory region of `offset..(offset
	/// + len)`, with 32 bytes as the step. If the length is zero, this function
	/// does nothing.
	pub fn resize_offset(&mut self, offset: U256, len: U256) -> Result<(), ExitError> {
		if len == U256::zero() {
			return Ok(())
		}

		if let Some(end) = offset.checked_add(len) {
			self.resize_end(end)
		} else {
			Err(ExitError::InvalidRange)
		}
	}

	/// Resize the memory, making it cover to `end`, with 32 bytes as the step.
	pub fn resize_end(&mut self, mut end: U256) -> Result<(), ExitError> {
		while end % U256::from(32) != U256::zero() {
			end = match end.checked_add(U256::one()) {
				Some(end) => end,
				None => return Err(ExitError::InvalidRange)
			};
		}

		self.effective_len = max(self.effective_len, end);
		Ok(())
	}

	/// Get memory region at given offset.
	///
	/// ## Panics
	///
	/// Value of `size` is considered trusted. If they're too large,
	/// the program can run out of memory, or it can overflow.
	pub fn get(&self, offset: usize, size: usize) -> Vec<u8> {
		let mut ret = Vec::new();
		ret.resize(size, 0);

		for index in 0..size {
			let position = offset + index;
			if position >= self.data.len() {
				break
			}

			ret[index] = self.data[position];
		}

		ret
	}

	/// Set memory region at given offset. The offset and value is considered
	/// untrusted.
	pub fn set(
		&mut self,
		offset: usize,
		value: &[u8],
		target_size: Option<usize>
	) -> Result<(), ExitFatal> {
		if value.is_empty() {
			return Ok(())
		}
		let target_size = target_size.unwrap_or(value.len());

		if offset.checked_add(target_size)
			.map(|pos| pos > self.limit).unwrap_or(true)
		{
			return Err(ExitFatal::NotSupported)
		}

		if self.data.len() < offset + target_size {
			self.data.resize(offset + target_size, 0);
		}

		for index in 0..target_size {
			if self.data.len() > offset + index && value.len() > index {
				self.data[offset + index] = value[index];
			} else {
				self.data[offset + index] = 0;
			}
		}

		Ok(())
	}

	/// Copy `data` into the memory, of given `len`.
	pub fn copy_large(
		&mut self,
		memory_offset: U256,
		data_offset: U256,
		len: U256,
		data: &[u8]
	) -> Result<(), ExitFatal> {
		let memory_offset = if memory_offset > U256::from(usize::max_value()) {
			return Err(ExitFatal::NotSupported)
		} else {
			memory_offset.as_usize()
		};

		let ulen = if len > U256::from(usize::max_value()) {
			return Err(ExitFatal::NotSupported)
		} else {
			len.as_usize()
		};

		let data = if let Some(end) = data_offset.checked_add(len) {
			if end > U256::from(usize::max_value()) {
				&[]
			} else {
				let data_offset = data_offset.as_usize();
				let end = end.as_usize();

				if data_offset > data.len() {
					&[]
				} else {
					&data[data_offset..min(end, data.len())]
				}
			}
		} else {
			&[]
		};

		self.set(memory_offset, data, Some(ulen))
	}
}
