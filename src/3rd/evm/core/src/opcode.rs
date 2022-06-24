/// Opcode enum. One-to-one corresponding to an `u8` value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Opcode(pub u8);

// Core opcodes.
impl Opcode {
	/// `STOP`
	pub const STOP: Opcode = Opcode(0x00);
	/// `ADD`
	pub const ADD: Opcode = Opcode(0x01);
	/// `MUL`
	pub const MUL: Opcode = Opcode(0x02);
	/// `SUB`
	pub const SUB: Opcode = Opcode(0x03);
	/// `DIV`
	pub const DIV: Opcode = Opcode(0x04);
	/// `SDIV`
	pub const SDIV: Opcode = Opcode(0x05);
	/// `MOD`
	pub const MOD: Opcode = Opcode(0x06);
	/// `SMOD`
	pub const SMOD: Opcode = Opcode(0x07);
	/// `ADDMOD`
	pub const ADDMOD: Opcode = Opcode(0x08);
	/// `MULMOD`
	pub const MULMOD: Opcode = Opcode(0x09);
	/// `EXP`
	pub const EXP: Opcode = Opcode(0x0a);
	/// `SIGNEXTEND`
	pub const SIGNEXTEND: Opcode = Opcode(0x0b);

	/// `LT`
	pub const LT: Opcode = Opcode(0x10);
	/// `GT`
	pub const GT: Opcode = Opcode(0x11);
	/// `SLT`
	pub const SLT: Opcode = Opcode(0x12);
	/// `SGT`
	pub const SGT: Opcode = Opcode(0x13);
	/// `EQ`
	pub const EQ: Opcode = Opcode(0x14);
	/// `ISZERO`
	pub const ISZERO: Opcode = Opcode(0x15);
	/// `AND`
	pub const AND: Opcode = Opcode(0x16);
	/// `OR`
	pub const OR: Opcode = Opcode(0x17);
	/// `XOR`
	pub const XOR: Opcode = Opcode(0x18);
	/// `NOT`
	pub const NOT: Opcode = Opcode(0x19);
	/// `BYTE`
	pub const BYTE: Opcode = Opcode(0x1a);

	/// `CALLDATALOAD`
	pub const CALLDATALOAD: Opcode = Opcode(0x35);
	/// `CALLDATASIZE`
	pub const CALLDATASIZE: Opcode = Opcode(0x36);
	/// `CALLDATACOPY`
	pub const CALLDATACOPY: Opcode = Opcode(0x37);
	/// `CODESIZE`
	pub const CODESIZE: Opcode = Opcode(0x38);
	/// `CODECOPY`
	pub const CODECOPY: Opcode = Opcode(0x39);

	/// `SHL`
	pub const SHL: Opcode = Opcode(0x1b);
	/// `SHR`
	pub const SHR: Opcode = Opcode(0x1c);
	/// `SAR`
	pub const SAR: Opcode = Opcode(0x1d);

	/// `POP`
	pub const POP: Opcode = Opcode(0x50);
	/// `MLOAD`
	pub const MLOAD: Opcode = Opcode(0x51);
	/// `MSTORE`
	pub const MSTORE: Opcode = Opcode(0x52);
	/// `MSTORE8`
	pub const MSTORE8: Opcode = Opcode(0x53);
	/// `JUMP`
	pub const JUMP: Opcode = Opcode(0x56);
	/// `JUMPI`
	pub const JUMPI: Opcode = Opcode(0x57);
	/// `PC`
	pub const PC: Opcode = Opcode(0x58);
	/// `MSIZE`
	pub const MSIZE: Opcode = Opcode(0x59);
	/// `JUMPDEST`
	pub const JUMPDEST: Opcode = Opcode(0x5b);

	/// `PUSHn`
	pub const PUSH1: Opcode = Opcode(0x60);
	pub const PUSH2: Opcode = Opcode(0x61);
	pub const PUSH3: Opcode = Opcode(0x62);
	pub const PUSH4: Opcode = Opcode(0x63);
	pub const PUSH5: Opcode = Opcode(0x64);
	pub const PUSH6: Opcode = Opcode(0x65);
	pub const PUSH7: Opcode = Opcode(0x66);
	pub const PUSH8: Opcode = Opcode(0x67);
	pub const PUSH9: Opcode = Opcode(0x68);
	pub const PUSH10: Opcode = Opcode(0x69);
	pub const PUSH11: Opcode = Opcode(0x6a);
	pub const PUSH12: Opcode = Opcode(0x6b);
	pub const PUSH13: Opcode = Opcode(0x6c);
	pub const PUSH14: Opcode = Opcode(0x6d);
	pub const PUSH15: Opcode = Opcode(0x6e);
	pub const PUSH16: Opcode = Opcode(0x6f);
	pub const PUSH17: Opcode = Opcode(0x70);
	pub const PUSH18: Opcode = Opcode(0x71);
	pub const PUSH19: Opcode = Opcode(0x72);
	pub const PUSH20: Opcode = Opcode(0x73);
	pub const PUSH21: Opcode = Opcode(0x74);
	pub const PUSH22: Opcode = Opcode(0x75);
	pub const PUSH23: Opcode = Opcode(0x76);
	pub const PUSH24: Opcode = Opcode(0x77);
	pub const PUSH25: Opcode = Opcode(0x78);
	pub const PUSH26: Opcode = Opcode(0x79);
	pub const PUSH27: Opcode = Opcode(0x7a);
	pub const PUSH28: Opcode = Opcode(0x7b);
	pub const PUSH29: Opcode = Opcode(0x7c);
	pub const PUSH30: Opcode = Opcode(0x7d);
	pub const PUSH31: Opcode = Opcode(0x7e);
	pub const PUSH32: Opcode = Opcode(0x7f);

	/// `DUPn`
	pub const DUP1: Opcode = Opcode(0x80);
	pub const DUP2: Opcode = Opcode(0x81);
	pub const DUP3: Opcode = Opcode(0x82);
	pub const DUP4: Opcode = Opcode(0x83);
	pub const DUP5: Opcode = Opcode(0x84);
	pub const DUP6: Opcode = Opcode(0x85);
	pub const DUP7: Opcode = Opcode(0x86);
	pub const DUP8: Opcode = Opcode(0x87);
	pub const DUP9: Opcode = Opcode(0x88);
	pub const DUP10: Opcode = Opcode(0x89);
	pub const DUP11: Opcode = Opcode(0x8a);
	pub const DUP12: Opcode = Opcode(0x8b);
	pub const DUP13: Opcode = Opcode(0x8c);
	pub const DUP14: Opcode = Opcode(0x8d);
	pub const DUP15: Opcode = Opcode(0x8e);
	pub const DUP16: Opcode = Opcode(0x8f);

	/// `SWAPn`
	pub const SWAP1: Opcode = Opcode(0x90);
	pub const SWAP2: Opcode = Opcode(0x91);
	pub const SWAP3: Opcode = Opcode(0x92);
	pub const SWAP4: Opcode = Opcode(0x93);
	pub const SWAP5: Opcode = Opcode(0x94);
	pub const SWAP6: Opcode = Opcode(0x95);
	pub const SWAP7: Opcode = Opcode(0x96);
	pub const SWAP8: Opcode = Opcode(0x97);
	pub const SWAP9: Opcode = Opcode(0x98);
	pub const SWAP10: Opcode = Opcode(0x99);
	pub const SWAP11: Opcode = Opcode(0x9a);
	pub const SWAP12: Opcode = Opcode(0x9b);
	pub const SWAP13: Opcode = Opcode(0x9c);
	pub const SWAP14: Opcode = Opcode(0x9d);
	pub const SWAP15: Opcode = Opcode(0x9e);
	pub const SWAP16: Opcode = Opcode(0x9f);

	/// `RETURN`
	pub const RETURN: Opcode = Opcode(0xf3);
	/// `REVERT`
	pub const REVERT: Opcode = Opcode(0xfd);

	/// `INVALID`
	pub const INVALID: Opcode = Opcode(0xfe);
}

// External opcodes
impl Opcode {
	/// `SHA3`
	pub const SHA3: Opcode = Opcode(0x20);
	/// `ADDRESS`
	pub const ADDRESS: Opcode = Opcode(0x30);
	/// `BALANCE`
	pub const BALANCE: Opcode = Opcode(0x31);
	/// `SELFBALANCE`
	pub const SELFBALANCE: Opcode = Opcode(0x47);
	/// `ORIGIN`
	pub const ORIGIN: Opcode = Opcode(0x32);
	/// `CALLER`
	pub const CALLER: Opcode = Opcode(0x33);
	/// `CALLVALUE`
	pub const CALLVALUE: Opcode = Opcode(0x34);
	/// `GASPRICE`
	pub const GASPRICE: Opcode = Opcode(0x3a);
	/// `EXTCODESIZE`
	pub const EXTCODESIZE: Opcode = Opcode(0x3b);
	/// `EXTCODECOPY`
	pub const EXTCODECOPY: Opcode = Opcode(0x3c);
	/// `EXTCODEHASH`
	pub const EXTCODEHASH: Opcode = Opcode(0x3f);
	/// `RETURNDATASIZE`
	pub const RETURNDATASIZE: Opcode = Opcode(0x3d);
	/// `RETURNDATACOPY`
	pub const RETURNDATACOPY: Opcode = Opcode(0x3e);
	/// `BLOCKHASH`
	pub const BLOCKHASH: Opcode = Opcode(0x40);
	/// `COINBASE`
	pub const COINBASE: Opcode = Opcode(0x41);
	/// `TIMESTAMP`
	pub const TIMESTAMP: Opcode = Opcode(0x42);
	/// `NUMBER`
	pub const NUMBER: Opcode = Opcode(0x43);
	/// `DIFFICULTY`
	pub const DIFFICULTY: Opcode = Opcode(0x44);
	/// `GASLIMIT`
	pub const GASLIMIT: Opcode = Opcode(0x45);
	/// `SLOAD`
	pub const SLOAD: Opcode = Opcode(0x54);
	/// `SSTORE`
	pub const SSTORE: Opcode = Opcode(0x55);
	/// `GAS`
	pub const GAS: Opcode = Opcode(0x5a);
	/// `LOGn`
	pub const LOG0: Opcode = Opcode(0xa0);
	pub const LOG1: Opcode = Opcode(0xa1);
	pub const LOG2: Opcode = Opcode(0xa2);
	pub const LOG3: Opcode = Opcode(0xa3);
	pub const LOG4: Opcode = Opcode(0xa4);
	/// `CREATE`
	pub const CREATE: Opcode = Opcode(0xf0);
	/// `CREATE2`
	pub const CREATE2: Opcode = Opcode(0xf5);
	/// `CALL`
	pub const CALL: Opcode = Opcode(0xf1);
	/// `CALLCODE`
	pub const CALLCODE: Opcode = Opcode(0xf2);
	/// `DELEGATECALL`
	pub const DELEGATECALL: Opcode = Opcode(0xf4);
	/// `STATICCALL`
	pub const STATICCALL: Opcode = Opcode(0xfa);
	/// `SUICIDE`
	pub const SUICIDE: Opcode = Opcode(0xff);
	/// `CHAINID`
	pub const CHAINID: Opcode = Opcode(0x46);
}

impl Opcode {
	/// Whether the opcode is a push opcode.
	pub fn is_push(&self) -> Option<u8> {
		let value = self.0;
		if value >= 0x60 && value <= 0x7f {
			Some(value - 0x60 + 1)
		} else {
			None
		}
	}

	#[inline]
	pub const fn as_u8(&self) -> u8 {
		self.0
	}

	#[inline]
	pub const fn as_usize(&self) -> usize {
		self.0 as usize
	}
}
