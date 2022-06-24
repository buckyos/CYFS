use core::cmp::min;
use alloc::vec::Vec;
use primitive_types::{H256, U256};
use sha3::{Keccak256, Digest};
use crate::{Runtime, ExitError, Handler, Capture, Transfer, ExitReason,
			CreateScheme, CallScheme, Context, ExitSucceed, ExitFatal};
use super::Control;

pub fn sha3<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	pop_u256!(runtime, from, len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(from, len));
	let data = if len == U256::zero() {
		Vec::new()
	} else {
		let from = as_usize_or_fail!(from);
		let len = as_usize_or_fail!(len);

		runtime.machine.memory_mut().get(from, len)
	};

	let ret = Keccak256::digest(data.as_slice());
	push!(runtime, H256::from_slice(ret.as_slice()));

	Control::Continue
}

pub fn chainid<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.chain_id());

	Control::Continue
}

pub fn address<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	push!(runtime, runtime.context.address.into());

	Control::Continue
}

pub fn balance<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop!(runtime, address);
	push_u256!(runtime, U256::from(handler.balance(address.into())));

	Control::Continue
}

pub fn selfbalance<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, U256::from(handler.balance(runtime.context.address)));

	Control::Continue
}

pub fn origin<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push!(runtime, handler.origin().into());

	Control::Continue
}

pub fn caller<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	push!(runtime, runtime.context.caller.into());

	Control::Continue
}

pub fn callvalue<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	push_u256!(runtime, U256::from(runtime.context.apparent_value));

	Control::Continue
}

pub fn gasprice<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.gas_price());

	Control::Continue
}

pub fn extcodesize<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop!(runtime, address);
	push_u256!(runtime, handler.code_size(address.into()));

	Control::Continue
}

pub fn extcodehash<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop!(runtime, address);
	push!(runtime, handler.code_hash(address.into()));

	Control::Continue
}

pub fn extcodecopy<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop!(runtime, address);
	pop_u256!(runtime, memory_offset, code_offset, len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(memory_offset, len));
	match runtime.machine.memory_mut().copy_large(
		memory_offset,
		code_offset,
		len,
		&handler.code(address.into())
	) {
		Ok(()) => (),
		Err(e) => return Control::Exit(e.into()),
	};

	Control::Continue
}

pub fn returndatasize<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	let size = U256::from(runtime.return_data_buffer.len());
	push_u256!(runtime, size);

	Control::Continue
}

pub fn returndatacopy<H: Handler>(runtime: &mut Runtime) -> Control<H> {
	pop_u256!(runtime, memory_offset, data_offset, len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(memory_offset, len));
	if data_offset.checked_add(len)
		.map(|l| l > U256::from(runtime.return_data_buffer.len()))
		.unwrap_or(true)
	{
		return Control::Exit(ExitError::OutOfOffset.into())
	}

	match runtime.machine.memory_mut().copy_large(memory_offset, data_offset, len, &runtime.return_data_buffer) {
		Ok(()) => Control::Continue,
		Err(e) => Control::Exit(e.into()),
	}
}

pub fn blockhash<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop_u256!(runtime, number);
	push!(runtime, handler.block_hash(number));

	Control::Continue
}

pub fn coinbase<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push!(runtime, handler.block_coinbase().into());
	Control::Continue
}

pub fn timestamp<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.block_timestamp());
	Control::Continue
}

pub fn number<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.block_number());
	Control::Continue
}

pub fn difficulty<H: Handler>(runtime: &mut Runtime, _handler: &H) -> Control<H> {
	push_u256!(runtime, U256::default());
	Control::Continue
}

pub fn gaslimit<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.block_gas_limit());
	Control::Continue
}

pub fn sload<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	pop!(runtime, index);
	push!(runtime, handler.storage(runtime.context.address, index));

	Control::Continue
}

pub fn sstore<H: Handler>(runtime: &mut Runtime, handler: &mut H) -> Control<H> {
	pop!(runtime, index, value);
	match handler.set_storage(runtime.context.address, index, value) {
		Ok(()) => Control::Continue,
		Err(e) => Control::Exit(e.into()),
	}
}

pub fn gas<H: Handler>(runtime: &mut Runtime, handler: &H) -> Control<H> {
	push_u256!(runtime, handler.gas_left());

	Control::Continue
}

pub fn log<H: Handler>(runtime: &mut Runtime, n: u8, handler: &mut H) -> Control<H> {
	pop_u256!(runtime, offset, len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(offset, len));
	let data = if len == U256::zero() {
		Vec::new()
	} else {
		let offset = as_usize_or_fail!(offset);
		let len = as_usize_or_fail!(len);

		runtime.machine.memory().get(offset, len)
	};

	let mut topics = Vec::new();
	for _ in 0..(n as usize) {
		match runtime.machine.stack_mut().pop() {
			Ok(value) => { topics.push(value); }
			Err(e) => return Control::Exit(e.into()),
		}
	}

	match handler.log(runtime.context.address, topics, data) {
		Ok(()) => Control::Continue,
		Err(e) => Control::Exit(e.into()),
	}
}

pub fn suicide<H: Handler>(runtime: &mut Runtime, handler: &mut H) -> Control<H> {
	pop!(runtime, target);

	match handler.mark_delete(runtime.context.address, target.into()) {
		Ok(()) => (),
		Err(e) => return Control::Exit(e.into()),
	}

	Control::Exit(ExitSucceed::Suicided.into())
}

pub fn create<H: Handler>(
	runtime: &mut Runtime,
	is_create2: bool,
	handler: &mut H,
) -> Control<H> {
	runtime.return_data_buffer = Vec::new();

	pop_u256!(runtime, value, code_offset, len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(code_offset, len));
	let code = if len == U256::zero() {
		Vec::new()
	} else {
		let code_offset = as_usize_or_fail!(code_offset);
		let len = as_usize_or_fail!(len);

		runtime.machine.memory().get(code_offset, len)
	};

	let scheme = if is_create2 {
		pop!(runtime, salt);
		let code_hash = H256::from_slice(Keccak256::digest(&code).as_slice());
		CreateScheme::Create2 {
			caller: runtime.context.address,
			salt,
			code_hash,
		}
	} else {
		CreateScheme::Legacy {
			caller: runtime.context.address,
		}
	};

	match handler.create(runtime.context.address, scheme, value.as_u64(), code, None) {
		Capture::Exit((reason, address, return_data)) => {
			runtime.return_data_buffer = return_data;
			let create_address: H256 = address.map(|a| a.into()).unwrap_or_default();

			match reason {
				ExitReason::Succeed(_) => {
					push!(runtime, create_address.into());
					Control::Continue
				},
				ExitReason::Revert(_) => {
					push!(runtime, H256::default());
					Control::Continue
				},
				ExitReason::Error(_) => {
					push!(runtime, H256::default());
					Control::Continue
				},
				ExitReason::Fatal(e) => {
					push!(runtime, H256::default());
					Control::Exit(e.into())
				},
			}
		},
		Capture::Trap(interrupt) => {
			push!(runtime, H256::default());
			Control::CreateInterrupt(interrupt)
		},
	}
}

pub fn call<'config, H: Handler>(
	runtime: &mut Runtime,
	scheme: CallScheme,
	handler: &mut H,
) -> Control<H> {
	runtime.return_data_buffer = Vec::new();

	pop_u256!(runtime, gas);
	pop!(runtime, to);
	let gas = if gas > U256::from(u64::MAX) {
		None
	} else {
		Some(gas.as_u64())
	};

	let value = match scheme {
		CallScheme::Call | CallScheme::CallCode => {
			pop_u256!(runtime, value);
			value
		},
		CallScheme::DelegateCall | CallScheme::StaticCall => {
			U256::zero()
		},
	};

	pop_u256!(runtime, in_offset, in_len, out_offset, out_len);

	try_or_fail!(runtime.machine.memory_mut().resize_offset(in_offset, in_len));
	try_or_fail!(runtime.machine.memory_mut().resize_offset(out_offset, out_len));

	let input = if in_len == U256::zero() {
		Vec::new()
	} else {
		let in_offset = as_usize_or_fail!(in_offset);
		let in_len = as_usize_or_fail!(in_len);

		runtime.machine.memory().get(in_offset, in_len)
	};

	let context = match scheme {
		CallScheme::Call | CallScheme::StaticCall => Context {
			address: to.into(),
			caller: runtime.context.address,
			apparent_value: value.as_u64(),
		},
		CallScheme::CallCode => Context {
			address: runtime.context.address,
			caller: runtime.context.address,
			apparent_value: value.as_u64(),
		},
		CallScheme::DelegateCall => Context {
			address: runtime.context.address,
			caller: runtime.context.caller,
			apparent_value: runtime.context.apparent_value,
		},
	};

	let transfer = if scheme == CallScheme::Call {
		Some(Transfer {
			source: runtime.context.address,
			target: to.into(),
			value: value.as_u64()
		})
	} else if scheme == CallScheme::CallCode {
		Some(Transfer {
			source: runtime.context.address,
			target: runtime.context.address,
			value: value.as_u64()
		})
	} else {
		None
	};

	match handler.call(to.into(), transfer, input, gas, scheme == CallScheme::StaticCall, context) {
		Capture::Exit((reason, return_data)) => {
			runtime.return_data_buffer = return_data;
			let target_len = min(out_len, U256::from(runtime.return_data_buffer.len()));

			match reason {
				ExitReason::Succeed(_) => {
					match runtime.machine.memory_mut().copy_large(
						out_offset,
						U256::zero(),
						target_len,
						&runtime.return_data_buffer[..],
					) {
						Ok(()) => {
							push_u256!(runtime, U256::one());
							Control::Continue
						},
						Err(_) => {
							push_u256!(runtime, U256::zero());
							Control::Continue
						},
					}
				},
				ExitReason::Revert(_) => {
					push_u256!(runtime, U256::zero());

					let _ = runtime.machine.memory_mut().copy_large(
						out_offset,
						U256::zero(),
						target_len,
						&runtime.return_data_buffer[..],
					);

					Control::Continue
				},
				ExitReason::Error(_) => {
					push_u256!(runtime, U256::zero());

					Control::Continue
				},
				ExitReason::Fatal(e) => {
					push_u256!(runtime, U256::zero());

					Control::Exit(e.into())
				},
			}
		},
		Capture::Trap(interrupt) => {
			push!(runtime, H256::default());
			Control::CallInterrupt(interrupt)
		},
	}
}
