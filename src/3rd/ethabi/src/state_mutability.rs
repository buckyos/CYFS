use serde::{
	de::{Error, Visitor},
	Deserialize, Deserializer,
};
use std::fmt;

/// Whether a function modifies or reads blockchain state
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StateMutability {
	/// Specified not to read blockchain state
	Pure,
	/// Specified to not modify the blockchain state
	View,
	/// Function does not accept Ether - the default
	NonPayable,
	/// Function accepts Ether
	Payable,
}

impl Default for StateMutability {
	fn default() -> Self {
		Self::NonPayable
	}
}

impl<'a> Deserialize<'a> for StateMutability {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'a>,
	{
		deserializer.deserialize_any(StateMutabilityVisitor)
	}
}

struct StateMutabilityVisitor;

impl<'a> Visitor<'a> for StateMutabilityVisitor {
	type Value = StateMutability;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		write!(formatter, "the string 'pure', 'view', 'payable', or 'nonpayable'")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: Error,
	{
		use StateMutability::*;
		Ok(match v {
			"pure" => Pure,
			"view" => View,
			"payable" => Payable,
			"nonpayable" => NonPayable,
			_ => return Err(Error::unknown_variant(v, &["pure", "view", "payable", "nonpayable"])),
		})
	}
}
