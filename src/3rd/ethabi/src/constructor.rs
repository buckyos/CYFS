// Copyright 2015-2020 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Contract constructor call builder.
use crate::{encode, Bytes, Error, Param, ParamType, Result, Token, LenientTokenizer, StrictTokenizer, Tokenizer};
use serde::Deserialize;

/// Contract constructor specification.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Constructor {
	/// Constructor input.
	pub inputs: Vec<Param>,
}

impl Constructor {
	/// Returns all input params of given constructor.
	fn param_types(&self) -> Vec<ParamType> {
		self.inputs.iter().map(|p| p.kind.clone()).collect()
	}

	fn parse_token(&self, input: &[String], lenient: bool) -> Result<Vec<Token>> {
		let params = self.param_types();
		if params.len() != input.len() {
			return Err(Error::InvalidData);
		}

		params.iter().zip(input).map(|(param, input)| match lenient {
			true => LenientTokenizer::tokenize(param, input),
			false => StrictTokenizer::tokenize(param, input),
		})
			.collect::<Result<_>>()
			.map_err(From::from)
	}

	/// Prepares ABI constructor call with given input params.
	pub fn encode_constructor(&self, code: Bytes, input: &[String], lenient: bool) -> Result<Bytes> {
		let token = self.parse_token(input, lenient)?;
		// Ok(encode(&tokens))
		Ok(code.into_iter().chain(encode(&token)).collect())
	}
}
