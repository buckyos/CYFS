use core::ops::Rem;
use core::convert::TryInto;
use primitive_types::{U256, U512};
use crate::utils::I256;

#[inline]
pub fn div(op1: U256, op2: U256) -> U256 {
	if op2 == U256::zero() {
		U256::zero()
	} else {
		op1 / op2
	}
}

#[inline]
pub fn sdiv(op1: U256, op2: U256) -> U256 {
	let op1: I256 = op1.into();
	let op2: I256 = op2.into();
	let ret = op1 / op2;
	ret.into()
}

#[inline]
pub fn rem(op1: U256, op2: U256) -> U256 {
	if op2 == U256::zero() {
		U256::zero()
	} else {
		op1.rem(op2)
	}
}

#[inline]
pub fn srem(op1: U256, op2: U256) -> U256 {
	if op2 == U256::zero() {
		U256::zero()
	} else {
		let op1: I256 = op1.into();
		let op2: I256 = op2.into();
		let ret = op1.rem(op2);
		ret.into()
	}
}

#[inline]
pub fn addmod(op1: U256, op2: U256, op3: U256) -> U256 {
	let op1: U512 = op1.into();
	let op2: U512 = op2.into();
	let op3: U512 = op3.into();

	if op3 == U512::zero() {
		U256::zero()
	} else {
		let v = (op1 + op2) % op3;
		v.try_into().expect("op3 is less than U256::max_value(), thus it never overflows; qed")
	}
}

#[inline]
pub fn mulmod(op1: U256, op2: U256, op3: U256) -> U256 {
	let op1: U512 = op1.into();
	let op2: U512 = op2.into();
	let op3: U512 = op3.into();

	if op3 == U512::zero() {
		U256::zero()
	} else {
		let v = (op1 * op2) % op3;
		v.try_into().expect("op3 is less than U256::max_value(), thus it never overflows; qed")
	}
}

#[inline]
pub fn exp(op1: U256, op2: U256) -> U256 {
	let mut op1 = op1;
	let mut op2 = op2;
	let mut r: U256 = 1.into();

	while op2 != 0.into() {
		if op2 & 1.into() != 0.into() {
			r = r.overflowing_mul(op1).0;
		}
		op2 = op2 >> 1;
		op1 = op1.overflowing_mul(op1).0;
	}

	r
}

#[inline]
pub fn signextend(op1: U256, op2: U256) -> U256 {
	if op1 > U256::from(32) {
		op2
	} else {
		let mut ret = U256::zero();
		let len: usize = op1.as_usize();
		let t: usize = 8 * (len + 1) - 1;
		let t_bit_mask = U256::one() << t;
		let t_value = (op2 & t_bit_mask) >> t;
		for i in 0..256 {
			let bit_mask = U256::one() << i;
			let i_value = (op2 & bit_mask) >> i;
			if i <= t {
				ret = ret.overflowing_add(i_value << i).0;
			} else {
				ret = ret.overflowing_add(t_value << i).0;
			}
		}
		ret
	}
}
