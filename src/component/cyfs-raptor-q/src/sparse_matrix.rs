use crate::arraymap::{ImmutableListMap, ImmutableListMapBuilder};
use crate::iterators::OctetIter;
use crate::matrix::BinaryMatrix;
use crate::octet::Octet;
use crate::sparse_vec::SparseBinaryVec;
use crate::util::get_both_indices;
use std::mem::size_of;

// Stores a matrix in sparse representation, with an optional dense block for the right most columns
// The logical storage is as follows:
// |---------------------------------------|
// |                          | (optional) |
// |      sparse rows         | dense      |
// |                          | columns    |
// |---------------------------------------|
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct SparseBinaryMatrix {
    height: usize,
    width: usize,
    sparse_elements: Vec<SparseBinaryVec>,
    // Note these are stored such that the right-most 64 elements of a row are in
    // dense_elements[row], the second 64 elements are stored in dense_elements[height + row], then
    // the next in dense_elements[height * 2 + row]. Elements are numbered right to left,
    // so the right-most element is in dense_elements[row] & 0b1. The second right most is in
    // dense_elements[row] & 0b2.
    dense_elements: Vec<u64>,
    // Columnar storage of values. Only stores rows that have a 1-valued entry in the given column
    sparse_columnar_values: Option<ImmutableListMap>,
    // Mapping of logical row numbers to index in sparse_elements, dense_elements, and sparse_column_index
    logical_row_to_physical: Vec<u32>,
    physical_row_to_logical: Vec<u32>,
    logical_col_to_physical: Vec<u16>,
    physical_col_to_logical: Vec<u16>,
    column_index_disabled: bool,
    // Only include for debug to avoid taking up extra memory in the cache
    #[cfg(debug_assertions)]
    debug_indexed_column_valid: Vec<bool>,
    num_dense_columns: usize,
}

const WORD_WIDTH: usize = 64;

impl SparseBinaryMatrix {
    #[cfg(debug_assertions)]
    fn verify(&self) {
        if self.column_index_disabled {
            return;
        }
        let columns = self.sparse_columnar_values.as_ref().unwrap();
        for row in 0..self.height {
            for (col, value) in self.sparse_elements[row].keys_values() {
                if value != Octet::zero() {
                    debug_assert!(columns.get(col as u16).contains(&(row as u32)));
                }
            }
        }
    }

    // Returns (word in elements vec, and bit in word) for the given col
    fn bit_position(&self, row: usize, col: usize) -> (usize, usize) {
        return (self.height * (col / WORD_WIDTH) + row, col % WORD_WIDTH);
    }

    // Return the word in which bit lives
    fn word_offset(bit: usize) -> usize {
        bit / WORD_WIDTH
    }

    // Returns mask to select the given bit in a word
    fn select_mask(bit: usize) -> u64 {
        1u64 << (bit as u64)
    }

    fn clear_bit(word: &mut u64, bit: usize) {
        *word &= !SparseBinaryMatrix::select_mask(bit);
    }

    fn set_bit(word: &mut u64, bit: usize) {
        *word |= SparseBinaryMatrix::select_mask(bit);
    }
}

impl BinaryMatrix for SparseBinaryMatrix {
    fn new(height: usize, width: usize, trailing_dense_column_hint: usize) -> SparseBinaryMatrix {
        debug_assert!(height < 16777216);
        // Matrix width can never exceed maximum L
        debug_assert!(width < 65536);
        let mut col_mapping = vec![0; width];
        let elements = vec![SparseBinaryVec::with_capacity(10); height];
        let mut row_mapping = vec![0; height];
        #[allow(clippy::needless_range_loop)]
        for i in 0..height {
            row_mapping[i] = i as u32;
        }
        #[allow(clippy::needless_range_loop)]
        for i in 0..width {
            col_mapping[i] = i as u16;
        }
        let dense_elements = if trailing_dense_column_hint > 0 {
            vec![0; height * ((trailing_dense_column_hint - 1) / WORD_WIDTH + 1)]
        } else {
            vec![]
        };
        SparseBinaryMatrix {
            height,
            width,
            sparse_elements: elements,
            dense_elements,
            sparse_columnar_values: None,
            logical_row_to_physical: row_mapping.clone(),
            physical_row_to_logical: row_mapping,
            logical_col_to_physical: col_mapping.clone(),
            physical_col_to_logical: col_mapping,
            column_index_disabled: true,
            num_dense_columns: trailing_dense_column_hint,
            #[cfg(debug_assertions)]
            debug_indexed_column_valid: vec![true; width],
        }
    }

    fn set(&mut self, i: usize, j: usize, value: Octet) {
        let physical_i = self.logical_row_to_physical[i] as usize;
        let physical_j = self.logical_col_to_physical[j] as usize;
        if self.width - j <= self.num_dense_columns {
            let (word, bit) = self.bit_position(physical_i, self.width - j - 1);
            if value == Octet::zero() {
                SparseBinaryMatrix::clear_bit(&mut self.dense_elements[word], bit);
            } else {
                SparseBinaryMatrix::set_bit(&mut self.dense_elements[word], bit);
            }
        } else {
            self.sparse_elements[physical_i].insert(physical_j, value);
            assert!(self.column_index_disabled);
        }
    }

    fn height(&self) -> usize {
        self.height
    }

    fn width(&self) -> usize {
        self.width
    }

    fn count_ones(&self, row: usize, start_col: usize, end_col: usize) -> usize {
        if end_col > self.width - self.num_dense_columns {
            unimplemented!("It was assumed that this wouldn't be needed, because the method would only be called on the V section of matrix A");
        }
        let mut ones = 0;
        let physical_row = self.logical_row_to_physical[row] as usize;
        for (physical_col, value) in self.sparse_elements[physical_row].keys_values() {
            let col = self.physical_col_to_logical[physical_col] as usize;
            if col >= start_col && col < end_col && value == Octet::one() {
                ones += 1;
            }
        }
        return ones;
    }

    fn get_sub_row_as_octets(&self, row: usize, start_col: usize) -> Vec<u8> {
        let first_dense_column = self.width - self.num_dense_columns;
        assert!(start_col >= first_dense_column);
        let mut result = Vec::with_capacity(self.width - start_col);
        for col in start_col..self.width {
            result.push(self.get(row, col).byte());
        }

        result
    }

    fn get(&self, i: usize, j: usize) -> Octet {
        let physical_i = self.logical_row_to_physical[i] as usize;
        let physical_j = self.logical_col_to_physical[j] as usize;
        if self.width - j <= self.num_dense_columns {
            let (word, bit) = self.bit_position(physical_i, self.width - j - 1);
            if self.dense_elements[word] & SparseBinaryMatrix::select_mask(bit) == 0 {
                return Octet::zero();
            } else {
                return Octet::one();
            }
        } else {
            return self.sparse_elements[physical_i]
                .get(physical_j)
                .unwrap_or_else(Octet::zero);
        }
    }

    fn get_row_iter(&self, row: usize, start_col: usize, end_col: usize) -> OctetIter {
        if end_col > self.width - self.num_dense_columns {
            unimplemented!("It was assumed that this wouldn't be needed, because the method would only be called on the V section of matrix A");
        }
        let physical_row = self.logical_row_to_physical[row] as usize;
        let sparse_elements = &self.sparse_elements[physical_row];
        OctetIter::new_sparse(
            start_col,
            end_col,
            sparse_elements,
            &self.physical_col_to_logical,
        )
    }

    fn get_ones_in_column(&self, col: usize, start_row: usize, end_row: usize) -> Vec<u32> {
        assert_eq!(self.column_index_disabled, false);
        #[cfg(debug_assertions)]
        debug_assert!(self.debug_indexed_column_valid[col]);
        let physical_col = self.logical_col_to_physical[col];
        let mut rows = vec![];
        for physical_row in self
            .sparse_columnar_values
            .as_ref()
            .unwrap()
            .get(physical_col)
        {
            let logical_row = self.physical_row_to_logical[*physical_row as usize];
            if start_row <= logical_row as usize && logical_row < end_row as u32 {
                rows.push(logical_row);
            }
        }

        rows
    }

    fn swap_rows(&mut self, i: usize, j: usize) {
        let physical_i = self.logical_row_to_physical[i] as usize;
        let physical_j = self.logical_row_to_physical[j] as usize;
        self.logical_row_to_physical.swap(i, j);
        self.physical_row_to_logical.swap(physical_i, physical_j);
    }

    fn swap_columns(&mut self, i: usize, j: usize, _: usize) {
        if j >= self.width - self.num_dense_columns {
            unimplemented!("It was assumed that this wouldn't be needed, because the method would only be called on the V section of matrix A");
        }

        #[cfg(debug_assertions)]
        self.debug_indexed_column_valid.swap(i, j);

        let physical_i = self.logical_col_to_physical[i] as usize;
        let physical_j = self.logical_col_to_physical[j] as usize;
        self.logical_col_to_physical.swap(i, j);
        self.physical_col_to_logical.swap(physical_i, physical_j);
    }

    fn enable_column_acccess_acceleration(&mut self) {
        self.column_index_disabled = false;
        let mut builder = ImmutableListMapBuilder::new(self.height);
        for (physical_row, elements) in self.sparse_elements.iter().enumerate() {
            for (physical_col, _) in elements.keys_values() {
                builder.add(physical_col as u16, physical_row as u32);
            }
        }
        self.sparse_columnar_values = Some(builder.build());
    }

    fn disable_column_acccess_acceleration(&mut self) {
        self.column_index_disabled = true;
        self.sparse_columnar_values = None;
    }

    fn hint_column_dense_and_frozen(&mut self, i: usize) {
        assert_eq!(
            self.width - self.num_dense_columns - 1,
            i,
            "Can only freeze the last sparse column"
        );
        assert_eq!(self.column_index_disabled, false);
        self.num_dense_columns += 1;
        let (last_word, last_bit) = self.bit_position(self.height, self.num_dense_columns - 1);
        // If this is in a new word
        if last_bit == 0 && last_word >= self.dense_elements.len() {
            // Append a new set of words
            self.dense_elements.extend(vec![0; self.height]);
        }
        let physical_i = self.logical_col_to_physical[i] as usize;
        for maybe_present_in_row in self
            .sparse_columnar_values
            .as_ref()
            .unwrap()
            .get(physical_i as u16)
        {
            let physical_row = *maybe_present_in_row as usize;
            if let Some(value) = self.sparse_elements[physical_row].remove(physical_i) {
                let (word, bit) = self.bit_position(physical_row, self.num_dense_columns - 1);
                if value == Octet::zero() {
                    SparseBinaryMatrix::clear_bit(&mut self.dense_elements[word], bit);
                } else {
                    SparseBinaryMatrix::set_bit(&mut self.dense_elements[word], bit);
                }
            }
        }
    }

    // other must be a rows x rows matrix
    // sets self[0..rows][..] = X * self[0..rows][..]
    fn mul_assign_submatrix(&mut self, other: &SparseBinaryMatrix, rows: usize) {
        assert_eq!(rows, other.height());
        assert_eq!(rows, other.width());
        assert!(rows <= self.height());
        assert!(self.column_index_disabled);
        if other.num_dense_columns != 0 {
            unimplemented!();
        }
        // Note: rows are logically indexed
        let mut temp_sparse = vec![SparseBinaryVec::with_capacity(10); rows];
        let mut temp_dense = vec![0; rows * ((self.num_dense_columns - 1) / WORD_WIDTH + 1)];
        for row in 0..rows {
            for (i, scalar) in other.get_row_iter(row, 0, rows) {
                let physical_i = self.logical_row_to_physical[i] as usize;
                if scalar != Octet::zero() {
                    temp_sparse[row].add_assign(&self.sparse_elements[physical_i]);
                    let words = SparseBinaryMatrix::word_offset(self.num_dense_columns - 1) + 1;
                    for word in 0..words {
                        let (src_word, _) = self.bit_position(physical_i, word * WORD_WIDTH);
                        temp_dense[word * rows + row] ^= self.dense_elements[src_word];
                    }
                }
            }
        }
        for row in (0..rows).rev() {
            let physical_row = self.logical_row_to_physical[row] as usize;
            self.sparse_elements[physical_row] = temp_sparse.pop().unwrap();
            let words = SparseBinaryMatrix::word_offset(self.num_dense_columns - 1) + 1;
            for word in 0..words {
                let (dest_word, _) = self.bit_position(physical_row, word * WORD_WIDTH);
                self.dense_elements[dest_word] = temp_dense[word * rows + row];
            }
        }

        #[cfg(debug_assertions)]
        self.verify();
    }

    fn add_assign_rows(&mut self, dest: usize, src: usize) {
        assert_ne!(dest, src);
        let physical_dest = self.logical_row_to_physical[dest] as usize;
        let physical_src = self.logical_row_to_physical[src] as usize;
        // First handle the dense columns
        if self.num_dense_columns > 0 {
            let words = SparseBinaryMatrix::word_offset(self.num_dense_columns - 1) + 1;
            for word in 0..words {
                let (dest_word, _) = self.bit_position(physical_dest, word * WORD_WIDTH);
                let (src_word, _) = self.bit_position(physical_src, word * WORD_WIDTH);
                self.dense_elements[dest_word] ^= self.dense_elements[src_word];
            }
        }

        // Then the sparse columns
        let (dest_row, temp_row) =
            get_both_indices(&mut self.sparse_elements, physical_dest, physical_src);
        // This shouldn't be needed, because while column indexing is enabled in first phase,
        // columns are only eliminated one at a time in sparse section of matrix.
        assert!(self.column_index_disabled || temp_row.len() == 1);

        let column_added = dest_row.add_assign(temp_row);
        // This shouldn't be needed, because while column indexing is enabled in first phase,
        // columns are only removed.
        assert!(self.column_index_disabled || !column_added);

        #[cfg(debug_assertions)]
        {
            if !self.column_index_disabled {
                let col = self.physical_col_to_logical[temp_row.get_by_raw_index(0).0];
                self.debug_indexed_column_valid[col as usize] = false;
            }
        }

        #[cfg(debug_assertions)]
        self.verify();
    }

    fn resize(&mut self, new_height: usize, new_width: usize) {
        assert!(new_height <= self.height);
        // Only support same width or removing all the dense columns
        let mut columns_to_remove = self.width - new_width;
        assert!(columns_to_remove == 0 || columns_to_remove >= self.num_dense_columns);
        if !self.column_index_disabled {
            unimplemented!(
                "Resize should only be used in phase 2, after column indexing is no longer needed"
            );
        }
        let mut new_sparse = vec![None; new_height];
        for i in (0..self.sparse_elements.len()).rev() {
            let logical_row = self.physical_row_to_logical[i] as usize;
            let sparse = self.sparse_elements.pop();
            if logical_row < new_height {
                new_sparse[logical_row] = sparse;
            }
        }

        if columns_to_remove == 0 && self.num_dense_columns > 0 {
            // TODO: optimize to not allocate this extra vec
            let mut new_dense =
                vec![0; new_height * ((self.num_dense_columns - 1) / WORD_WIDTH + 1)];
            let words = SparseBinaryMatrix::word_offset(self.num_dense_columns - 1) + 1;
            for word in 0..words {
                for logical_row in 0..new_height {
                    let physical_row = self.logical_row_to_physical[logical_row] as usize;
                    new_dense[word * new_height + logical_row] =
                        self.dense_elements[word * self.height + physical_row];
                }
            }
            self.dense_elements = new_dense;
        } else {
            columns_to_remove -= self.num_dense_columns;
            self.dense_elements.clear();
            self.num_dense_columns = 0;
        }

        self.logical_row_to_physical.truncate(new_height);
        self.physical_row_to_logical.truncate(new_height);
        for i in 0..new_height {
            self.logical_row_to_physical[i] = i as u32;
            self.physical_row_to_logical[i] = i as u32;
        }
        for row in new_sparse.drain(0..new_height) {
            self.sparse_elements.push(row.unwrap());
        }

        // Next remove sparse columns
        if columns_to_remove > 0 {
            let physical_to_logical = &self.physical_col_to_logical;
            for row in 0..self.sparse_elements.len() {
                self.sparse_elements[row]
                    .retain(|(col, _)| physical_to_logical[*col] < new_width as u16);
            }
        }

        self.height = new_height;
        self.width = new_width;

        #[cfg(debug_assertions)]
        self.verify();
    }

    fn size_in_bytes(&self) -> usize {
        let mut bytes = size_of::<Self>();
        for x in self.sparse_elements.iter() {
            bytes += x.size_in_bytes();
        }
        bytes += size_of::<u64>() * self.dense_elements.len();
        if let Some(ref columns) = self.sparse_columnar_values {
            bytes += columns.size_in_bytes();
        }
        bytes += size_of::<u32>() * self.logical_row_to_physical.len();
        bytes += size_of::<u32>() * self.physical_row_to_logical.len();
        bytes += size_of::<u16>() * self.logical_col_to_physical.len();
        bytes += size_of::<u16>() * self.physical_col_to_logical.len();
        #[cfg(debug_assertions)]
        {
            bytes += size_of::<bool>() * self.debug_indexed_column_valid.len();
        }

        bytes
    }
}

#[cfg(test)]
mod tests {
    use crate::systematic_constants::{num_intermediate_symbols, MAX_SOURCE_SYMBOLS_PER_BLOCK};

    #[test]
    fn check_max_width_optimization() {
        // Check that the optimization of limiting matrix width to 2^16 is safe.
        // Matrix width will never exceed L
        assert!(num_intermediate_symbols(MAX_SOURCE_SYMBOLS_PER_BLOCK) < 65536);
    }
}
