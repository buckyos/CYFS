use crate::octet::Octet;
use crate::octets::add_assign;
use crate::octets::fused_addassign_mul_scalar;
use crate::octets::mulassign_scalar;
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};
use std::ops::AddAssign;

/// Elementary unit of data, for encoding/decoding purposes.
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Symbol {
    value: Vec<u8>,
}

impl Symbol {
    pub fn new(value: Vec<u8>) -> Symbol {
        Symbol { value }
    }

    /// Initialize a zeroed symbol, with given size.
    pub fn zero<T>(size: T) -> Symbol
    where
        T: Into<usize>,
    {
        Symbol {
            value: vec![0; size.into()],
        }
    }

    #[cfg(feature = "benchmarking")]
    pub fn len(&self) -> usize {
        self.value.len()
    }

    /// Return the underlying byte slice for a symbol.
    pub fn as_bytes(&self) -> &[u8] {
        &self.value
    }

    /// Consume a symbol into a vector of bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.value
    }

    pub fn mulassign_scalar(&mut self, scalar: &Octet) {
        mulassign_scalar(&mut self.value, scalar);
    }

    pub fn fused_addassign_mul_scalar(&mut self, other: &Symbol, scalar: &Octet) {
        fused_addassign_mul_scalar(&mut self.value, &other.value, scalar);
    }
}

impl<'a> AddAssign<&'a Symbol> for Symbol {
    fn add_assign(&mut self, other: &'a Symbol) {
        add_assign(&mut self.value, &other.value);
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use crate::symbol::Symbol;

    #[test]
    fn add_assign() {
        let symbol_size = 41;
        let mut data1: Vec<u8> = vec![0; symbol_size];
        let mut data2: Vec<u8> = vec![0; symbol_size];
        let mut result: Vec<u8> = vec![0; symbol_size];
        for i in 0..symbol_size {
            data1[i] = rand::thread_rng().gen();
            data2[i] = rand::thread_rng().gen();
            result[i] = data1[i] ^ data2[i];
        }
        let mut symbol1 = Symbol::new(data1);
        let symbol2 = Symbol::new(data2);

        symbol1 += &symbol2;
        assert_eq!(result, symbol1.into_bytes());
    }
}
