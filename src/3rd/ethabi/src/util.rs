// Copyright 2015-2020 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utils used by different modules.

use crate::Word;

/// Converts a u32 to a right aligned array of 32 bytes.
pub fn pad_u32(value: u32) -> Word {
	let mut padded = [0u8; 32];
	padded[28..32].copy_from_slice(&value.to_be_bytes());
	padded
}

#[cfg(test)]
mod tests {
	use super::pad_u32;
	use hex_literal::hex;

	#[test]
	fn test_pad_u32() {
		// this will fail if endianness is not supported
		assert_eq!(
			pad_u32(0).to_vec(),
			hex!("0000000000000000000000000000000000000000000000000000000000000000").to_vec()
		);
		assert_eq!(
			pad_u32(1).to_vec(),
			hex!("0000000000000000000000000000000000000000000000000000000000000001").to_vec()
		);
		assert_eq!(
			pad_u32(0x100).to_vec(),
			hex!("0000000000000000000000000000000000000000000000000000000000000100").to_vec()
		);
		assert_eq!(
			pad_u32(0xffffffff).to_vec(),
			hex!("00000000000000000000000000000000000000000000000000000000ffffffff").to_vec()
		);
	}
}
