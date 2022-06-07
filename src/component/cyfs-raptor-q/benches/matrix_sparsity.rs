use cyfs_raptorq::generate_constraint_matrix;
use cyfs_raptorq::IntermediateSymbolDecoder;
use cyfs_raptorq::Octet;
use cyfs_raptorq::Symbol;
use cyfs_raptorq::{extended_source_block_symbols, BinaryMatrix, SparseBinaryMatrix};

fn main() {
    for elements in [10, 100, 1000, 10000, 40000, 56403].iter() {
        let num_symbols = extended_source_block_symbols(*elements);
        let indices: Vec<u32> = (0..num_symbols).collect();
        let (a, hdpc) = generate_constraint_matrix::<SparseBinaryMatrix>(num_symbols, &indices);
        let mut density = 0;
        let mut row_density = vec![0; a.height()];
        for i in 0..a.height() {
            for j in 0..a.width() {
                if i < a.height() - hdpc.height() {
                    if a.get(i, j) != Octet::zero() {
                        density += 1;
                        row_density[i] += 1;
                    }
                } else {
                    if hdpc.get(i - (a.height() - hdpc.height()), j) != Octet::zero() {
                        density += 1;
                        row_density[i] += 1;
                    }
                }
            }
        }
        row_density.sort();
        let min = row_density[0];
        let max = row_density[row_density.len() - 1];
        let p50 = row_density[(row_density.len() as f64 * 0.5) as usize];
        let p80 = row_density[(row_density.len() as f64 * 0.8) as usize];
        let p90 = row_density[(row_density.len() as f64 * 0.9) as usize];
        let p95 = row_density[(row_density.len() as f64 * 0.95) as usize];
        let p99 = row_density[(row_density.len() as f64 * 0.99) as usize];
        println!(
            "Row density for {}x{}: min={} max={} p50={} p80={} p90={} p95={} p99={}",
            a.height(),
            a.width(),
            min,
            max,
            p50,
            p80,
            p90,
            p95,
            p99
        );
        println!(
            "Original density for {}x{}: {} of {} ({:.3}%)",
            a.height(),
            a.width(),
            density,
            a.height() * a.width(),
            100.0 * density as f64 / (a.height() * a.width()) as f64
        );

        let symbols = vec![Symbol::zero(1usize); a.width()];
        let mut decoder = IntermediateSymbolDecoder::new(a, hdpc, symbols, num_symbols);
        println!(
            "Initial memory usage: {}KB",
            decoder.get_non_symbol_bytes() / 1024
        );
        decoder.execute();
        println!(
            "Optimized decoder mul ops: {} ({:.1} per symbol), add ops: {} ({:.1} per symbol)",
            decoder.get_symbol_mul_ops(),
            decoder.get_symbol_mul_ops() as f64 / num_symbols as f64,
            decoder.get_symbol_add_ops(),
            decoder.get_symbol_add_ops() as f64 / num_symbols as f64
        );
        println!(
            "By phase mul ops: {:?}, add ops: {:?}",
            decoder.get_symbol_mul_ops_by_phase(),
            decoder.get_symbol_add_ops_by_phase()
        );
        println!();
    }
}
