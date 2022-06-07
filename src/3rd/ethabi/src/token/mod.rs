// Copyright 2015-2020 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! ABI param and parsing for it.

mod lenient;
mod strict;
mod token;

use std::cmp::Ordering::{Equal, Less};

pub use self::{lenient::LenientTokenizer, strict::StrictTokenizer, token::Token};
use crate::{Error, ParamType};
use cyfs_base::ObjectId;

/// This trait should be used to parse string values as tokens.
pub trait Tokenizer {
	/// Tries to parse a string as a token of given type.
	fn tokenize(param: &ParamType, value: &str) -> Result<Token, Error> {
		match *param {
			ParamType::Address => Self::tokenize_address(value).map(|a| Token::Address(a)),
			ParamType::String => Self::tokenize_string(value).map(Token::String),
			ParamType::Bool => Self::tokenize_bool(value).map(Token::Bool),
			ParamType::Bytes => Self::tokenize_bytes(value).map(Token::Bytes),
			ParamType::FixedBytes(len) => Self::tokenize_fixed_bytes(value, len).map(Token::FixedBytes),
			ParamType::Uint(_) => Self::tokenize_uint(value).map(Into::into).map(Token::Uint),
			ParamType::Int(_) => Self::tokenize_int(value).map(Into::into).map(Token::Int),
			ParamType::Array(ref p) => Self::tokenize_array(value, p).map(Token::Array),
			ParamType::FixedArray(ref p, len) => Self::tokenize_fixed_array(value, p, len).map(Token::FixedArray),
			ParamType::Tuple(ref p) => Self::tokenize_struct(value, p).map(Token::Tuple),
		}
	}

	/// Tries to parse a value as a vector of tokens of fixed size.
	fn tokenize_fixed_array(value: &str, param: &ParamType, len: usize) -> Result<Vec<Token>, Error> {
		let result = Self::tokenize_array(value, param)?;
		match result.len() == len {
			true => Ok(result),
			false => Err(Error::InvalidData),
		}
	}

	/// Tried to parse a struct as a vector of tokens
	fn tokenize_struct(value: &str, param: &[ParamType]) -> Result<Vec<Token>, Error> {
		if !value.starts_with('(') || !value.ends_with(')') {
			return Err(Error::InvalidData);
		}

		if value.chars().count() == 2 {
			return Ok(vec![]);
		}

		let mut result = vec![];
		let mut nested = 0isize;
		let mut ignore = false;
		let mut last_item = 1;
		let mut params = param.iter();
		for (pos, ch) in value.chars().enumerate() {
			match ch {
				'(' if !ignore => {
					nested += 1;
				}
				')' if !ignore => {
					nested -= 1;

					match nested.cmp(&0) {
						Less => {
							return Err(Error::InvalidData);
						}
						Equal => {
							let sub = &value[last_item..pos];
							let token = Self::tokenize(params.next().ok_or(Error::InvalidData)?, sub)?;
							result.push(token);
							last_item = pos + 1;
						}
						_ => {}
					}
				}
				'"' => {
					ignore = !ignore;
				}
				',' if nested == 1 && !ignore => {
					let sub = &value[last_item..pos];
					let token = Self::tokenize(params.next().ok_or(Error::InvalidData)?, sub)?;
					result.push(token);
					last_item = pos + 1;
				}
				_ => (),
			}
		}

		if ignore {
			return Err(Error::InvalidData);
		}

		Ok(result)
	}

	/// Tries to parse a value as a vector of tokens.
	fn tokenize_array(value: &str, param: &ParamType) -> Result<Vec<Token>, Error> {
		if !value.starts_with('[') || !value.ends_with(']') {
			return Err(Error::InvalidData);
		}

		if value.chars().count() == 2 {
			return Ok(vec![]);
		}

		let mut result = vec![];
		let mut nested = 0isize;
		let mut ignore = false;
		let mut last_item = 1;
		for (i, ch) in value.chars().enumerate() {
			match ch {
				'[' if !ignore => {
					nested += 1;
				}
				']' if !ignore => {
					nested -= 1;
					match nested.cmp(&0) {
						Less => {
							return Err(Error::InvalidData);
						}
						Equal => {
							let sub = &value[last_item..i];
							let token = Self::tokenize(param, sub)?;
							result.push(token);
							last_item = i + 1;
						}
						_ => {}
					}
				}
				'"' => {
					ignore = !ignore;
				}
				',' if nested == 1 && !ignore => {
					let sub = &value[last_item..i];
					let token = Self::tokenize(param, sub)?;
					result.push(token);
					last_item = i + 1;
				}
				_ => (),
			}
		}

		if ignore {
			return Err(Error::InvalidData);
		}

		Ok(result)
	}

	/// Tries to parse a value as an address.
	fn tokenize_address(value: &str) -> Result<ObjectId, Error>;

	/// Tries to parse a value as a string.
	fn tokenize_string(value: &str) -> Result<String, Error>;

	/// Tries to parse a value as a bool.
	fn tokenize_bool(value: &str) -> Result<bool, Error>;

	/// Tries to parse a value as bytes.
	fn tokenize_bytes(value: &str) -> Result<Vec<u8>, Error>;

	/// Tries to parse a value as bytes.
	fn tokenize_fixed_bytes(value: &str, len: usize) -> Result<Vec<u8>, Error>;

	/// Tries to parse a value as unsigned integer.
	fn tokenize_uint(value: &str) -> Result<[u8; 32], Error>;

	/// Tries to parse a value as signed integer.
	fn tokenize_int(value: &str) -> Result<[u8; 32], Error>;
}

#[cfg(test)]
mod test {
	use super::{LenientTokenizer, ParamType, Tokenizer};
	#[test]
	fn single_quoted_in_array_must_error() {
		assert!(LenientTokenizer::tokenize_array("[1,\"0,false]", &ParamType::Bool).is_err());
		assert!(LenientTokenizer::tokenize_array("[false\"]", &ParamType::Bool).is_err());
		assert!(LenientTokenizer::tokenize_array("[1,false\"]", &ParamType::Bool).is_err());
		assert!(LenientTokenizer::tokenize_array("[1,\"0\",false]", &ParamType::Bool).is_err());
		assert!(LenientTokenizer::tokenize_array("[1,0]", &ParamType::Bool).is_ok());
	}
}
