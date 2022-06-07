use crate::matrix::DenseBinaryMatrix;
use crate::octet::Octet;
use crate::sparse_vec::SparseBinaryVec;

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct ClonedOctetIter {
    sparse: bool,
    end_col: usize,
    dense_elements: Option<Vec<u64>>,
    dense_index: usize,
    sparse_elements: Option<Vec<(usize, Octet)>>,
    sparse_index: usize,
}

impl Iterator for ClonedOctetIter {
    type Item = (usize, Octet);

    fn next(&mut self) -> Option<Self::Item> {
        if self.sparse {
            let elements = self.sparse_elements.as_ref().unwrap();
            if self.sparse_index == elements.len() {
                return None;
            } else {
                let old_index = self.sparse_index;
                self.sparse_index += 1;
                return Some(elements[old_index].clone());
            }
        } else if self.dense_index == self.end_col {
            return None;
        } else {
            let old_index = self.dense_index;
            self.dense_index += 1;
            let (word, bit) = DenseBinaryMatrix::bit_position(old_index);
            let value = if self.dense_elements.as_ref().unwrap()[word]
                & DenseBinaryMatrix::select_mask(bit)
                == 0
            {
                Octet::zero()
            } else {
                Octet::one()
            };
            return Some((old_index, value));
        }
    }
}

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct OctetIter<'a> {
    sparse: bool,
    start_col: usize,
    end_col: usize,
    dense_elements: Option<&'a Vec<u64>>,
    dense_index: usize,
    sparse_elements: Option<&'a SparseBinaryVec>,
    sparse_index: usize,
    sparse_physical_col_to_logical: Option<&'a [u16]>,
}

impl<'a> OctetIter<'a> {
    pub fn new_sparse(
        start_col: usize,
        end_col: usize,
        sparse_elements: &'a SparseBinaryVec,
        sparse_physical_col_to_logical: &'a [u16],
    ) -> OctetIter<'a> {
        OctetIter {
            sparse: true,
            start_col,
            end_col,
            dense_elements: None,
            dense_index: 0,
            sparse_elements: Some(sparse_elements),
            sparse_index: 0,
            sparse_physical_col_to_logical: Some(sparse_physical_col_to_logical),
        }
    }

    #[allow(clippy::ptr_arg)]
    pub fn new_dense_binary(
        start_col: usize,
        end_col: usize,
        dense_elements: &'a Vec<u64>,
    ) -> OctetIter<'a> {
        OctetIter {
            sparse: false,
            start_col: 0,
            end_col,
            dense_elements: Some(dense_elements),
            dense_index: start_col,
            sparse_elements: None,
            sparse_index: 0,
            sparse_physical_col_to_logical: None,
        }
    }

    pub fn clone(&self) -> ClonedOctetIter {
        // Convert to logical indices, since ClonedOctetIter doesn't handle physical
        let sparse_elements = self.sparse_elements.map(|x| {
            x.keys_values()
                .map(|(physical_col, value)| {
                    (
                        self.sparse_physical_col_to_logical.unwrap()[physical_col] as usize,
                        value,
                    )
                })
                .filter(|(logical_col, _)| {
                    *logical_col >= self.start_col && *logical_col < self.end_col
                })
                .collect()
        });
        ClonedOctetIter {
            sparse: self.sparse,
            end_col: self.end_col,
            dense_elements: self.dense_elements.cloned(),
            dense_index: self.dense_index,
            sparse_elements,
            sparse_index: self.sparse_index,
        }
    }
}

impl<'a> Iterator for OctetIter<'a> {
    type Item = (usize, Octet);

    fn next(&mut self) -> Option<Self::Item> {
        if self.sparse {
            let elements = self.sparse_elements.unwrap();
            // Need to iterate over the whole array, since they're not sorted by logical col
            if self.sparse_index >= elements.len() {
                return None;
            } else {
                while self.sparse_index < elements.len() {
                    let entry = elements.get_by_raw_index(self.sparse_index);
                    self.sparse_index += 1;
                    let logical_col = self.sparse_physical_col_to_logical.unwrap()[entry.0];
                    if logical_col >= self.start_col as u16 && logical_col < self.end_col as u16 {
                        return Some((logical_col as usize, entry.1));
                    }
                }
                return None;
            }
        } else if self.dense_index == self.end_col {
            return None;
        } else {
            let old_index = self.dense_index;
            self.dense_index += 1;
            let (word, bit) = DenseBinaryMatrix::bit_position(old_index);
            let value =
                if self.dense_elements.unwrap()[word] & DenseBinaryMatrix::select_mask(bit) == 0 {
                    Octet::zero()
                } else {
                    Octet::one()
                };
            return Some((old_index, value));
        }
    }
}
