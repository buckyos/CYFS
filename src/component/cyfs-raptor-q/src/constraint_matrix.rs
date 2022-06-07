use crate::base::intermediate_tuple;
use crate::matrix::BinaryMatrix;
use crate::octet::Octet;
use crate::octet_matrix::DenseOctetMatrix;
use crate::octets::{add_assign, fused_addassign_mul_scalar};
use crate::rng::rand;
use crate::systematic_constants::extended_source_block_symbols;
use crate::systematic_constants::num_hdpc_symbols;
use crate::systematic_constants::num_intermediate_symbols;
use crate::systematic_constants::num_ldpc_symbols;
use crate::systematic_constants::num_lt_symbols;
use crate::systematic_constants::num_pi_symbols;
use crate::systematic_constants::{calculate_p1, systematic_index};

// Simulates Enc[] function to get indices of accessed intermediate symbols, as defined in section 5.3.5.3
#[allow(clippy::many_single_char_names)]
pub fn enc_indices(
    source_tuple: (u32, u32, u32, u32, u32, u32),
    lt_symbols: u32,
    pi_symbols: u32,
    p1: u32,
) -> Vec<usize> {
    let w = lt_symbols;
    let p = pi_symbols;
    let (d, a, mut b, d1, a1, mut b1) = source_tuple;

    assert!(d > 0);
    assert!(1 <= a && a < w);
    assert!(b < w);
    assert!(d1 == 2 || d1 == 3);
    assert!(1 <= a1 && a1 < p1);
    assert!(b1 < p1);

    let mut indices = Vec::with_capacity((d + d1) as usize);
    indices.push(b as usize);

    for _ in 1..d {
        b = (b + a) % w;
        indices.push(b as usize);
    }

    while b1 >= p {
        b1 = (b1 + a1) % p1;
    }

    indices.push((w + b1) as usize);

    for _ in 1..d1 {
        b1 = (b1 + a1) % p1;
        while b1 >= p {
            b1 = (b1 + a1) % p1;
        }
        indices.push((w + b1) as usize);
    }

    indices
}

#[allow(non_snake_case)]
fn generate_hdpc_rows(Kprime: usize, S: usize, H: usize) -> DenseOctetMatrix {
    let mut matrix = DenseOctetMatrix::new(H, Kprime + S + H, 0);
    // G_HDPC

    // Generates the MT matrix
    // See section 5.3.3.3
    let mut mt: Vec<Vec<u8>> = vec![vec![0; Kprime + S]; H];
    #[allow(clippy::needless_range_loop)]
    for i in 0..H {
        #[allow(clippy::needless_range_loop)]
        for j in 0..=(Kprime + S - 2) {
            let rand6 = rand((j + 1) as u32, 6u32, H as u32) as usize;
            let rand7 = rand((j + 1) as u32, 7u32, (H - 1) as u32) as usize;
            if i == rand6 || i == (rand6 + rand7 + 1) % H {
                mt[i][j] = 1;
            }
        }
        mt[i][Kprime + S - 1] = Octet::alpha(i).byte();
    }
    // Multiply by the GAMMA matrix
    // See section 5.3.3.3
    let mut gamma_row = vec![0; Kprime + S];
    // We only create the last row of the GAMMA matrix, as all preceding rows are just a shift left
    #[allow(clippy::needless_range_loop)]
    for j in 0..(Kprime + S) {
        // The spec says "alpha ^^ (i-j)". However, this clearly can overflow since alpha() is
        // only defined up to input < 256. Since alpha() only has 255 unique values, we must
        // take the input mod 255. Without this the constraint matrix ends up being singular
        // for 1698 and 8837 source symbols.
        gamma_row[j] = Octet::alpha((Kprime + S - 1 - j) % 255).byte();
    }
    #[allow(clippy::needless_range_loop)]
    for i in 0..H {
        let mut result_row = vec![0; Kprime + S];
        for j in 0..(Kprime + S) {
            let scalar = Octet::new(mt[i][j]);
            if scalar == Octet::zero() {
                continue;
            }
            if scalar == Octet::one() {
                add_assign(
                    &mut result_row[0..=j],
                    &gamma_row[(Kprime + S - j - 1)..(Kprime + S)],
                );
            } else {
                fused_addassign_mul_scalar(
                    &mut result_row[0..=j],
                    &gamma_row[(Kprime + S - j - 1)..(Kprime + S)],
                    &scalar,
                );
            }
        }
        #[allow(clippy::needless_range_loop)]
        for j in 0..(Kprime + S) {
            if result_row[j] != 0 {
                matrix.set(i, j, Octet::new(result_row[j]));
            }
        }
    }

    // I_H
    for i in 0..H {
        matrix.set(i, i + (Kprime + S) as usize, Octet::one());
    }

    matrix
}

// See section 5.3.3.4.2
// Returns the HDPC rows separately. These logically replace the rows S..(S + H) of the constraint
// matrix. They are returned separately to allow easier optimizations.
#[allow(non_snake_case)]
pub fn generate_constraint_matrix<T: BinaryMatrix>(
    source_block_symbols: u32,
    encoded_symbol_indices: &[u32],
) -> (T, DenseOctetMatrix) {
    let Kprime = extended_source_block_symbols(source_block_symbols) as usize;
    let S = num_ldpc_symbols(source_block_symbols) as usize;
    let H = num_hdpc_symbols(source_block_symbols) as usize;
    let W = num_lt_symbols(source_block_symbols) as usize;
    let B = W - S;
    let P = num_pi_symbols(source_block_symbols) as usize;
    let L = num_intermediate_symbols(source_block_symbols) as usize;

    assert!(S + H + encoded_symbol_indices.len() >= L);
    let mut matrix = T::new(S + H + encoded_symbol_indices.len(), L, P);

    // G_LDPC,1
    // See section 5.3.3.3
    for i in 0..B {
        let a = 1 + i / S;

        let b = i % S;
        matrix.set(b, i, Octet::one());

        let b = (b + a) % S;
        matrix.set(b, i, Octet::one());

        let b = (b + a) % S;
        matrix.set(b, i, Octet::one());
    }

    // I_S
    for i in 0..S {
        matrix.set(i as usize, i + B as usize, Octet::one());
    }

    // G_LDPC,2
    // See section 5.3.3.3
    for i in 0..S {
        matrix.set(i, (i % P) + W, Octet::one());
        matrix.set(i, ((i + 1) % P) + W, Octet::one());
    }

    // G_ENC
    let lt_symbols = num_lt_symbols(Kprime as u32);
    let pi_symbols = num_pi_symbols(Kprime as u32);
    let sys_index = systematic_index(Kprime as u32);
    let p1 = calculate_p1(Kprime as u32);
    for (row, &i) in encoded_symbol_indices.iter().enumerate() {
        // row != i, because i is the ESI
        let tuple = intermediate_tuple(i, lt_symbols, sys_index, p1);

        for j in enc_indices(tuple, lt_symbols, pi_symbols, p1) {
            matrix.set(row as usize + S + H, j, Octet::one());
        }
    }

    (matrix, generate_hdpc_rows(Kprime, S, H))
}
