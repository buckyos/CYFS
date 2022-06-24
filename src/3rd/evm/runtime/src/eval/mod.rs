#[macro_use]
mod macros;
mod system;

use crate::{Handler, Runtime, ExitReason, CallScheme, Opcode};

pub enum Control<H: Handler> {
	Continue,
	CallInterrupt(H::CallInterrupt),
	CreateInterrupt(H::CreateInterrupt),
	Exit(ExitReason)
}

fn handle_other<H: Handler>(state: &mut Runtime, opcode: Opcode, handler: &mut H) -> Control<H> {
	match handler.other(
		opcode,
		&mut state.machine
	) {
		Ok(()) => Control::Continue,
		Err(e) => Control::Exit(e.into()),
	}
}

pub fn eval<H: Handler>(state: &mut Runtime, opcode: Opcode, handler: &mut H) -> Control<H> {
	match opcode {
		Opcode::SHA3 => system::sha3(state),
		Opcode::ADDRESS => system::address(state),
		Opcode::BALANCE => system::balance(state, handler),
		Opcode::SELFBALANCE => system::selfbalance(state, handler),
		Opcode::ORIGIN => system::origin(state, handler),
		Opcode::CALLER => system::caller(state),
		Opcode::CALLVALUE => system::callvalue(state),
		Opcode::GASPRICE => system::gasprice(state, handler),
		Opcode::EXTCODESIZE => system::extcodesize(state, handler),
		Opcode::EXTCODEHASH => system::extcodehash(state, handler),
		Opcode::EXTCODECOPY => system::extcodecopy(state, handler),
		Opcode::RETURNDATASIZE => system::returndatasize(state),
		Opcode::RETURNDATACOPY => system::returndatacopy(state),
		Opcode::BLOCKHASH => system::blockhash(state, handler),
		Opcode::COINBASE => system::coinbase(state, handler),
		Opcode::TIMESTAMP => system::timestamp(state, handler),
		Opcode::NUMBER => system::number(state, handler),
		Opcode::DIFFICULTY => system::difficulty(state, handler),
		Opcode::GASLIMIT => system::gaslimit(state, handler),
		Opcode::SLOAD => system::sload(state, handler),
		Opcode::SSTORE => system::sstore(state, handler),
		Opcode::GAS => system::gas(state, handler),
		Opcode::LOG0 => system::log(state, 0, handler),
		Opcode::LOG1 => system::log(state, 1, handler),
		Opcode::LOG2 => system::log(state, 2, handler),
		Opcode::LOG3 => system::log(state, 3, handler),
		Opcode::LOG4 => system::log(state, 4, handler),
		Opcode::SUICIDE => system::suicide(state, handler),
		Opcode::CREATE => system::create(state, false, handler),
		Opcode::CREATE2 => system::create(state, true, handler),
		Opcode::CALL => system::call(state, CallScheme::Call, handler),
		Opcode::CALLCODE => system::call(state, CallScheme::CallCode, handler),
		Opcode::DELEGATECALL => system::call(state, CallScheme::DelegateCall, handler),
		Opcode::STATICCALL => system::call(state, CallScheme::StaticCall, handler),
		Opcode::CHAINID => system::chainid(state, handler),
		_ => handle_other(state, opcode, handler),
	}
}
