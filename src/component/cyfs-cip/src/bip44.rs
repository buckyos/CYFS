use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};

use std::str::FromStr;

const HARDENED_BIT: u32 = 1 << 31;

/// A child number for a derived key
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ChildNumber(u32);

impl ChildNumber {
	pub fn is_hardened(&self) -> bool {
		self.0 & HARDENED_BIT == HARDENED_BIT
	}

	pub fn is_normal(&self) -> bool {
		self.0 & HARDENED_BIT == 0
	}

	pub fn to_bytes(&self) -> [u8; 4] {
		self.0.to_be_bytes()
	}
}

impl FromStr for ChildNumber {
    type Err = BuckyError;

    fn from_str(child: &str) -> BuckyResult<ChildNumber> {
    	let (child, mask) = if child.ends_with('\'') {
    		(&child[..child.len() - 1], HARDENED_BIT)
    	} else {
    		(child, 0)
    	};

        let index: u32 = child.parse().map_err(|e| {
			let msg = format!("parse child error: child={}, err={}", child, e);
			error!("{}", msg);

			BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
		})?;

        if index & HARDENED_BIT == 0 {
        	Ok(ChildNumber(index | mask))
        } else {
			let msg = format!("invalid child: child={}, index={}", child, index);
			error!("{}", msg);

			Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DerivationPath {
    path: Vec<ChildNumber>,
}

impl FromStr for DerivationPath {
    type Err = BuckyError;

    fn from_str(path: &str) -> BuckyResult<DerivationPath> {
        let mut spath = path.split('/');

        if spath.next() != Some("m") {
			let msg = format!("invalid path format: path={}", path);
			error!("{}", msg);

			return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(DerivationPath {
            path: spath.map(str::parse).collect::<BuckyResult<Vec<ChildNumber>>>()?
        })
    }
}

impl DerivationPath {
	pub fn as_ref(&self) -> &[ChildNumber] {
		&self.path
	}

	pub fn iter(&self) -> impl Iterator<Item = &ChildNumber> {
		self.path.iter()
	}
}

pub trait IntoDerivationPath {
	fn into(self) -> BuckyResult<DerivationPath>;
}

impl IntoDerivationPath for DerivationPath {
	fn into(self) -> BuckyResult<DerivationPath> {
		Ok(self)
	}
}

impl IntoDerivationPath for &str {
	fn into(self) -> BuckyResult<DerivationPath> {
		self.parse()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn derive_path() {
		let path: DerivationPath = "m/44'/60'/0'/0".parse().unwrap();

		assert_eq!(path, DerivationPath {
			path: vec![
				ChildNumber(44 | HARDENED_BIT),
				ChildNumber(60 | HARDENED_BIT),
				ChildNumber(0  | HARDENED_BIT),
				ChildNumber(0),
			],
		});
	}
}
