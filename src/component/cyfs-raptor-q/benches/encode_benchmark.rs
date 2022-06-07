use rand::Rng;
use cyfs_raptorq::{ObjectTransmissionInformation, SourceBlockEncoder, SourceBlockEncodingPlan};
use std::time::Instant;

const TARGET_TOTAL_BYTES: usize = 128 * 1024 * 1024;
const SYMBOL_COUNTS: [usize; 10] = [10, 100, 250, 500, 1000, 2000, 5000, 10000, 20000, 50000];

fn black_box(value: u64) {
    if value == rand::thread_rng().gen() {
        println!("{}", value);
    }
}

fn benchmark(symbol_size: u16, pre_plan: bool) -> u64 {
    let mut black_box_value = 0;
    for symbol_count in SYMBOL_COUNTS.iter() {
        let elements = symbol_count * symbol_size as usize;
        let mut data: Vec<u8> = vec![0; elements];
        for i in 0..elements {
            data[i] = rand::thread_rng().gen();
        }

        let plan = if pre_plan {
            Some(SourceBlockEncodingPlan::generate(*symbol_count as u16))
        } else {
            None
        };

        let now = Instant::now();
        let iterations = TARGET_TOTAL_BYTES / elements;
        let config = ObjectTransmissionInformation::new(0, symbol_size, 0, 1, 1);
        for _ in 0..iterations {
            let encoder = if let Some(ref plan) = plan {
                SourceBlockEncoder::with_encoding_plan2(1, &config, &data, plan)
            } else {
                SourceBlockEncoder::new2(1, &config, &data)
            };
            let packets = encoder.repair_packets(0, 1);
            black_box_value += packets[0].data()[0] as u64;
        }
        let elapsed = now.elapsed();
        let elapsed = elapsed.as_secs() as f64 + elapsed.subsec_millis() as f64 * 0.001;
        let throughput = (elements * iterations * 8) as f64 / 1024.0 / 1024.0 / elapsed;
        println!(
            "symbol count = {}, encoded {} MB in {:.3}secs, throughput: {:.1}Mbit/s",
            symbol_count,
            elements * iterations / 1024 / 1024,
            elapsed,
            throughput
        );
    }
    return black_box_value;
}

fn main() {
    let symbol_size = 1280;
    println!(
        "Symbol size: {} bytes (without pre-built plan)",
        symbol_size
    );
    black_box(benchmark(symbol_size, false));
    println!();
    println!("Symbol size: {} bytes (with pre-built plan)", symbol_size);
    black_box(benchmark(symbol_size, true));
}
