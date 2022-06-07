use rand::Rng;
use cyfs_raptorq::{ObjectTransmissionInformation, SourceBlockDecoder, SourceBlockEncoder};
use std::time::Instant;

const TARGET_TOTAL_BYTES: usize = 128 * 1024 * 1024;
const SYMBOL_COUNTS: [usize; 10] = [10, 100, 250, 500, 1000, 2000, 5000, 10000, 20000, 50000];

fn black_box(value: u64) {
    if value == rand::thread_rng().gen() {
        println!("{}", value);
    }
}

fn benchmark(symbol_size: u16, overhead: f64) -> u64 {
    let mut black_box_value = 0;
    for &symbol_count in SYMBOL_COUNTS.iter() {
        let elements = symbol_count * symbol_size as usize;
        let mut data: Vec<u8> = vec![0; elements];
        for i in 0..elements {
            data[i] = rand::thread_rng().gen();
        }

        let iterations = TARGET_TOTAL_BYTES / elements;
        let config = ObjectTransmissionInformation::new(0, symbol_size, 0, 1, 1);
        let encoder = SourceBlockEncoder::new2(1, &config, &data);
        let elements_and_overhead = (symbol_count as f64 * (1.0 + overhead)) as u32;
        let mut packets =
            encoder.repair_packets(0, (iterations as u32 * elements_and_overhead) as u32);
        let now = Instant::now();
        for _ in 0..iterations {
            let mut decoder = SourceBlockDecoder::new2(1, &config, elements as u64);
            let start = packets.len() - elements_and_overhead as usize;
            if let Some(result) = decoder.decode(packets.drain(start..)) {
                black_box_value += result[0] as u64;
            }
        }
        let elapsed = now.elapsed();
        let elapsed = elapsed.as_secs() as f64 + elapsed.subsec_millis() as f64 * 0.001;
        let throughput = (elements * iterations * 8) as f64 / 1024.0 / 1024.0 / elapsed;
        println!("symbol count = {}, decoded {} MB in {:.3}secs using {:.1}% overhead, throughput: {:.1}Mbit/s",
                 symbol_count,
                 elements * iterations / 1024 / 1024,
                 elapsed,
                 100.0 * overhead,
                 throughput);
    }

    return black_box_value;
}

fn main() {
    let symbol_size = 1280;
    println!("Symbol size: {} bytes", symbol_size);
    black_box(benchmark(symbol_size, 0.0));
    println!();
    black_box(benchmark(symbol_size, 0.05));
}
