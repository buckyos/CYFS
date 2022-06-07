use crate::octet::Octet;
use crate::symbol::Symbol;
use crate::util::get_both_indices;
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub enum SymbolOps {
    AddAssign {
        dest: usize,
        src: usize,
    },
    MulAssign {
        dest: usize,
        scalar: Octet,
    },
    FMA {
        dest: usize,
        src: usize,
        scalar: Octet,
    },
    Reorder {
        order: Vec<usize>,
    },
}

pub fn perform_op(op: &SymbolOps, symbols: &mut Vec<Symbol>) {
    match op {
        SymbolOps::AddAssign { dest, src } => {
            let (dest, temp) = get_both_indices(symbols, *dest, *src);
            *dest += temp;
        }
        SymbolOps::MulAssign { dest, scalar } => {
            symbols[*dest].mulassign_scalar(scalar);
        }
        SymbolOps::FMA { dest, src, scalar } => {
            let (dest, temp) = get_both_indices(symbols, *dest, *src);
            dest.fused_addassign_mul_scalar(temp, scalar);
        }
        SymbolOps::Reorder { order } => {
            /* TODO: Reorder is the last step of the algorithm. It should be
             *       possible to move reorder to be the first step and use when
             *       creating D (place all rows in correct position before
             *       calculations). This will however force an update on all
             *       row-numbers used in all other "Operations". */
            let mut temp_symbols: Vec<Option<Symbol>> = symbols.drain(..).map(Some).collect();
            for row_index in order.iter() {
                symbols.push(temp_symbols[*row_index].take().unwrap());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use crate::octet::Octet;
    use crate::operation_vector::{perform_op, SymbolOps};
    use crate::symbol::Symbol;

    #[test]
    fn test_add() {
        let rows = 2;
        let symbol_size = 1316;
        let mut data: Vec<Symbol> = Vec::with_capacity(rows);

        for _i in 0..rows {
            let mut symbol_data: Vec<u8> = vec![0; symbol_size];
            for j in 0..symbol_size {
                symbol_data[j] = rand::thread_rng().gen();
            }
            let symbol = Symbol::new(symbol_data);
            data.push(symbol);
        }

        let mut data0: Vec<u8> = vec![0; symbol_size];
        let mut data1: Vec<u8> = vec![0; symbol_size];
        let mut result: Vec<u8> = vec![0; symbol_size];
        for i in 0..symbol_size {
            data0[i] = data[0].as_bytes()[i];
            data1[i] = data[1].as_bytes()[i];
            result[i] = data0[i] ^ data1[i];
        }
        let mut symbol0 = Symbol::new(data0);
        let symbol1 = Symbol::new(data1);

        symbol0 += &symbol1;

        perform_op(&SymbolOps::AddAssign { dest: 0, src: 1 }, &mut data);
        assert_eq!(result, data[0].as_bytes());
    }

    #[test]
    fn test_add_mul() {
        let rows = 2;
        let symbol_size = 1316;
        let mut data: Vec<Symbol> = Vec::with_capacity(rows);

        for _i in 0..rows {
            let mut symbol_data: Vec<u8> = vec![0; symbol_size];
            for j in 0..symbol_size {
                symbol_data[j] = rand::thread_rng().gen();
            }
            let symbol = Symbol::new(symbol_data);
            data.push(symbol);
        }

        let value = 173;
        let mut data0: Vec<u8> = vec![0; symbol_size];
        let mut data1: Vec<u8> = vec![0; symbol_size];
        let mut result: Vec<u8> = vec![0; symbol_size];
        for i in 0..symbol_size {
            data0[i] = data[0].as_bytes()[i];
            data1[i] = data[1].as_bytes()[i];
            result[i] = data0[i] ^ (Octet::new(data1[i]) * Octet::new(value)).byte();
        }

        perform_op(
            &SymbolOps::FMA {
                dest: 0,
                src: 1,
                scalar: Octet::new(value),
            },
            &mut data,
        );
        assert_eq!(result, data[0].as_bytes());
    }

    #[test]
    fn test_mul() {
        let rows = 1;
        let symbol_size = 1316;
        let mut data: Vec<Symbol> = Vec::with_capacity(rows);

        for _i in 0..rows {
            let mut symbol_data: Vec<u8> = vec![0; symbol_size];
            for j in 0..symbol_size {
                symbol_data[j] = rand::thread_rng().gen();
            }
            let symbol = Symbol::new(symbol_data);
            data.push(symbol);
        }

        let value = 215;
        let mut data0: Vec<u8> = vec![0; symbol_size];
        let mut result: Vec<u8> = vec![0; symbol_size];
        for i in 0..symbol_size {
            data0[i] = data[0].as_bytes()[i];
            result[i] = (Octet::new(data0[i]) * Octet::new(value)).byte();
        }

        perform_op(
            &SymbolOps::MulAssign {
                dest: 0,
                scalar: Octet::new(value),
            },
            &mut data,
        );
        assert_eq!(result, data[0].as_bytes());
    }

    #[test]
    fn test_reorder() {
        let rows = 10;
        let symbol_size = 10;
        let mut data: Vec<Symbol> = Vec::with_capacity(rows);

        for i in 0..rows {
            let mut symbol_data: Vec<u8> = vec![0; symbol_size];
            for j in 0..symbol_size {
                symbol_data[j] = i as u8;
            }
            let symbol = Symbol::new(symbol_data);
            data.push(symbol);
        }

        assert_eq!(data[0].as_bytes()[0], 0);
        assert_eq!(data[1].as_bytes()[0], 1);
        assert_eq!(data[2].as_bytes()[0], 2);
        assert_eq!(data[9].as_bytes()[0], 9);

        perform_op(
            &SymbolOps::Reorder {
                order: vec![9, 7, 5, 3, 1, 8, 0, 6, 2, 4],
            },
            &mut data,
        );
        assert_eq!(data[0].as_bytes()[0], 9);
        assert_eq!(data[1].as_bytes()[0], 7);
        assert_eq!(data[2].as_bytes()[0], 5);
        assert_eq!(data[3].as_bytes()[0], 3);
        assert_eq!(data[4].as_bytes()[0], 1);
        assert_eq!(data[5].as_bytes()[0], 8);
        assert_eq!(data[6].as_bytes()[0], 0);
        assert_eq!(data[7].as_bytes()[0], 6);
        assert_eq!(data[8].as_bytes()[0], 2);
        assert_eq!(data[9].as_bytes()[0], 4);
    }
}
