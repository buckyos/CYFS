// Copyright 2015-2020 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use anyhow::anyhow;
use std::{num, string};
use thiserror::Error;

/// Ethabi result type
pub type Result<T> = std::result::Result<T, Error>;

/// Ethabi errors
#[derive(Debug, Error)]
pub enum Error {
	/// Invalid entity such as a bad function name.
	#[error("Invalid name: {0}")]
	InvalidName(String),
	/// Invalid data.
	#[error("Invalid data")]
	InvalidData,
	/// Serialization error.
	#[error("Serialization error: {0}")]
	SerdeJson(#[from] serde_json::Error),
	/// Integer parsing error.
	#[error("Integer parsing error: {0}")]
	ParseInt(#[from] num::ParseIntError),
	/// UTF-8 parsing error.
	#[error("UTF-8 parsing error: {0}")]
	Utf8(#[from] string::FromUtf8Error),
	/// Hex string parsing error.
	#[error("Hex parsing error: {0}")]
	Hex(#[from] hex::FromHexError),
	/// Hex string parsing error.
	#[error("Hex parsing error: {0}")]
	Hex2(#[from] rustc_hex::FromHexError),
	/// Other errors.
	#[error("{0}")]
	Other(#[from] anyhow::Error),
}

impl From<uint::FromDecStrErr> for Error {
	fn from(err: uint::FromDecStrErr) -> Self {
		use uint::FromDecStrErr::*;
		match err {
			InvalidCharacter => anyhow!("Uint parse error: InvalidCharacter"),
			InvalidLength => anyhow!("Uint parse error: InvalidLength"),
		}
		.into()
	}
}
