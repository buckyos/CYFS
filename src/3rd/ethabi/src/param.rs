// Copyright 2015-2020 Parity Technologies
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Function param.

use serde::{
	de::{Error, MapAccess, Visitor},
	Deserialize, Deserializer,
};
use std::fmt;

use crate::{ParamType, TupleParam};

/// Function param.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
	/// Param name.
	pub name: String,
	/// Param type.
	pub kind: ParamType,
}

impl<'a> Deserialize<'a> for Param {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'a>,
	{
		deserializer.deserialize_any(ParamVisitor)
	}
}

struct ParamVisitor;

impl<'a> Visitor<'a> for ParamVisitor {
	type Value = Param;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		write!(formatter, "a valid event parameter spec")
	}

	fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
	where
		V: MapAccess<'a>,
	{
		let mut name = None;
		let mut kind = None;
		let mut components = None;

		while let Some(ref key) = map.next_key::<String>()? {
			match key.as_ref() {
				"name" => {
					if name.is_some() {
						return Err(Error::duplicate_field("name"));
					}
					name = Some(map.next_value()?);
				}
				"type" => {
					if kind.is_some() {
						return Err(Error::duplicate_field("kind"));
					}
					kind = Some(map.next_value()?);
				}
				"components" => {
					if components.is_some() {
						return Err(Error::duplicate_field("components"));
					}
					let component: Vec<TupleParam> = map.next_value()?;
					components = Some(component)
				}
				_ => {}
			}
		}
		let name = name.ok_or_else(|| Error::missing_field("name"))?;
		let mut kind = kind.ok_or_else(|| Error::missing_field("kind"))?;
		set_tuple_components::<V::Error>(&mut kind, components)?;
		Ok(Param { name, kind })
	}
}

fn inner_tuple(mut param: &mut ParamType) -> Option<&mut Vec<ParamType>> {
	loop {
		match param {
			ParamType::Array(inner) => param = inner.as_mut(),
			ParamType::FixedArray(inner, _) => param = inner.as_mut(),
			ParamType::Tuple(inner) => return Some(inner),
			_ => return None,
		}
	}
}

pub fn set_tuple_components<Error: serde::de::Error>(
	kind: &mut ParamType,
	components: Option<Vec<TupleParam>>,
) -> Result<(), Error> {
	if let Some(inner_tuple) = inner_tuple(kind) {
		let tuple_params = components.ok_or_else(|| Error::missing_field("components"))?;
		inner_tuple.extend(tuple_params.into_iter().map(|param| param.kind))
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use crate::{Param, ParamType};

	#[test]
	fn param_deserialization() {
		let s = r#"{
			"name": "foo",
			"type": "address"
		}"#;

		let deserialized: Param = serde_json::from_str(s).unwrap();

		assert_eq!(deserialized, Param { name: "foo".to_owned(), kind: ParamType::Address });
	}

	#[test]
	fn param_tuple_deserialization() {
		let s = r#"{
			"name": "foo",
			"type": "tuple",
			"components": [
				{
					"name": "amount",
					"type": "uint48"
				},
				{
					"name": "things",
					"type": "tuple",
					"components": [
						{
							"name": "baseTupleParam",
							"type": "address"
						}
					]
				}
			]
		}"#;

		let deserialized: Param = serde_json::from_str(s).unwrap();

		assert_eq!(
			deserialized,
			Param {
				name: "foo".to_owned(),
				kind: ParamType::Tuple(vec![ParamType::Uint(48), ParamType::Tuple(vec![ParamType::Address])]),
			}
		);
	}

	#[test]
	fn param_tuple_array_deserialization() {
		let s = r#"{
			"name": "foo",
			"type": "tuple[]",
			"components": [
				{
					"name": "amount",
					"type": "uint48"
				},
				{
					"name": "to",
					"type": "address"
				},
				{
					"name": "from",
					"type": "address"
				}
			]
		}"#;

		let deserialized: Param = serde_json::from_str(s).unwrap();

		assert_eq!(
			deserialized,
			Param {
				name: "foo".to_owned(),
				kind: ParamType::Array(Box::new(ParamType::Tuple(vec![
					ParamType::Uint(48),
					ParamType::Address,
					ParamType::Address
				]))),
			}
		);
	}

	#[test]
	fn param_array_of_array_of_tuple_deserialization() {
		let s = r#"{
			"name": "foo",
			"type": "tuple[][]",
			"components": [
				{
					"name": "u0",
					"type": "uint8"
				},
				{
					"name": "u1",
					"type": "uint16"
				}
			]
		}"#;

		let deserialized: Param = serde_json::from_str(s).unwrap();
		assert_eq!(
			deserialized,
			Param {
				name: "foo".to_owned(),
				kind: ParamType::Array(Box::new(ParamType::Array(Box::new(ParamType::Tuple(vec![
					ParamType::Uint(8),
					ParamType::Uint(16),
				]))))),
			}
		);
	}

	#[test]
	fn param_tuple_fixed_array_deserialization() {
		let s = r#"{
			"name": "foo",
			"type": "tuple[2]",
			"components": [
				{
					"name": "amount",
					"type": "uint48"
				},
				{
					"name": "to",
					"type": "address"
				},
				{
					"name": "from",
					"type": "address"
				}
			]
		}"#;

		let deserialized: Param = serde_json::from_str(s).unwrap();

		assert_eq!(
			deserialized,
			Param {
				name: "foo".to_owned(),
				kind: ParamType::FixedArray(
					Box::new(ParamType::Tuple(vec![ParamType::Uint(48), ParamType::Address, ParamType::Address])),
					2
				),
			}
		);
	}

	#[test]
	fn param_tuple_with_nested_tuple_arrays_deserialization() {
		let s = r#"{
			"name": "foo",
			"type": "tuple",
			"components": [
				{
					"name": "bar",
					"type": "tuple[]",
					"components": [
						{
							"name": "a",
							"type": "address"
						}
					]
				},
				{
					"name": "baz",
					"type": "tuple[42]",
					"components": [
						{
							"name": "b",
							"type": "address"
						}
					]
				}
			]
		}"#;

		let deserialized: Param = serde_json::from_str(s).unwrap();

		assert_eq!(
			deserialized,
			Param {
				name: "foo".to_owned(),
				kind: ParamType::Tuple(vec![
					ParamType::Array(Box::new(ParamType::Tuple(vec![ParamType::Address]))),
					ParamType::FixedArray(Box::new(ParamType::Tuple(vec![ParamType::Address])), 42,)
				]),
			}
		);
	}
}
