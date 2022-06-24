// Copyright 2015-2020 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::{
	param_type::{ParamType, Writer},
	Hash,
};
use sha3::{Digest, Keccak256};

pub fn short_signature(name: &str, params: &[ParamType]) -> [u8; 4] {
	let mut result = [0u8; 4];
	fill_signature(name, params, &mut result);
	result
}

pub fn long_signature(name: &str, params: &[ParamType]) -> Hash {
	let mut result = [0u8; 32];
	fill_signature(name, params, &mut result);
	result.into()
}

fn fill_signature(name: &str, params: &[ParamType], result: &mut [u8]) {
	let types = params.iter().map(Writer::write).collect::<Vec<String>>().join(",");

	let data: Vec<u8> = From::from(format!("{}({})", name, types).as_str());

	result.copy_from_slice(&Keccak256::digest(&data)[..result.len()])
}

#[cfg(test)]
mod tests {
	use super::short_signature;
	use crate::ParamType;
	use hex_literal::hex;

	#[test]
	fn test_signature() {
		assert_eq!(hex!("cdcd77c0"), short_signature("baz", &[ParamType::Uint(32), ParamType::Bool]));
	}
}
