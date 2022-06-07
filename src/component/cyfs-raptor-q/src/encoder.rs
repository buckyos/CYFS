use crate::base::intermediate_tuple;
use crate::base::partition;
use crate::base::EncodingPacket;
use crate::base::PayloadId;
use crate::constraint_matrix::generate_constraint_matrix;
use crate::matrix::DenseBinaryMatrix;
use crate::operation_vector::{perform_op, SymbolOps};
use crate::pi_solver::fused_inverse_mul_symbols;
use crate::sparse_matrix::SparseBinaryMatrix;
use crate::symbol::Symbol;
use crate::systematic_constants::extended_source_block_symbols;
use crate::systematic_constants::num_hdpc_symbols;
use crate::systematic_constants::num_intermediate_symbols;
use crate::systematic_constants::num_ldpc_symbols;
use crate::systematic_constants::num_lt_symbols;
use crate::systematic_constants::num_pi_symbols;
use crate::systematic_constants::{calculate_p1, systematic_index};
use crate::ObjectTransmissionInformation;
#[cfg(feature = "serde_support")]
use serde::{Deserialize, Serialize};

pub const SPARSE_MATRIX_THRESHOLD: u32 = 250;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct EncoderBuilder {
    decoder_memory_requirement: u64,
    max_packet_size: u16,
}

impl EncoderBuilder {
    pub fn new() -> EncoderBuilder {
        EncoderBuilder {
            decoder_memory_requirement: 10 * 1024 * 1024,
            max_packet_size: 1024,
        }
    }

    pub fn set_decoder_memory_requirement(&mut self, bytes: u64) {
        self.decoder_memory_requirement = bytes;
    }

    pub fn set_max_packet_size(&mut self, bytes: u16) {
        self.max_packet_size = bytes;
    }

    pub fn build(&self, data: &[u8]) -> Encoder {
        let config = ObjectTransmissionInformation::generate_encoding_parameters(
            data.len() as u64,
            self.max_packet_size,
            self.decoder_memory_requirement,
        );

        Encoder::new(data, config)
    }
}

// Calculate the splits [start, end) of an object for encoding as blocks.
// If a block extends past the end of the object, it must be zero padded
pub fn calculate_block_offsets(
    data: &[u8],
    config: &ObjectTransmissionInformation,
) -> Vec<(usize, usize)> {
    let kt = (config.transfer_length() as f64 / config.symbol_size() as f64).ceil() as u32;
    let (kl, ks, zl, zs) = partition(kt, config.source_blocks());

    let mut data_index = 0;
    let mut blocks = vec![];
    if zl > 0 {
        for _ in 0..zl {
            let offset = kl as usize * config.symbol_size() as usize;
            blocks.push((data_index, (data_index + offset)));
            data_index += offset;
        }
    }

    if zs > 0 {
        for _ in zl..(zl + zs) {
            let offset = ks as usize * config.symbol_size() as usize;
            if data_index + offset <= data.len() {
                blocks.push((data_index, (data_index + offset)));
            } else {
                // Should only be possible when Kt * T > F. See third to last paragraph in section 4.4.1.2
                assert!(kt as usize * config.symbol_size() as usize > data.len());
                blocks.push((data_index, (data_index + offset)));
            }
            data_index += offset;
        }
    }

    blocks
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Encoder {
    config: ObjectTransmissionInformation,
    blocks: Vec<SourceBlockEncoder>,
}

impl Encoder {
    pub fn new(data: &[u8], config: ObjectTransmissionInformation) -> Encoder {
        let mut block_encoders = vec![];
        let mut cached_plan: Option<SourceBlockEncodingPlan> = None;
        for (i, (start, end)) in calculate_block_offsets(data, &config).drain(..).enumerate() {
            // Zero pad if necessary
            let mut padded;
            let block: &[u8] = if end > data.len() {
                padded = Vec::from(&data[start..]);
                padded.extend(vec![0; end - data.len()]);
                &padded
            } else {
                &data[start..end]
            };

            let symbol_count = block.len() / config.symbol_size() as usize;
            if cached_plan.is_none()
                || cached_plan.as_ref().unwrap().source_symbol_count != symbol_count as u16
            {
                let plan = SourceBlockEncodingPlan::generate(symbol_count as u16);
                cached_plan = Some(plan);
            }
            block_encoders.push(SourceBlockEncoder::with_encoding_plan2(
                i as u8,
                &config,
                &block,
                cached_plan.as_ref().unwrap(),
            ));
        }

        Encoder {
            config,
            blocks: block_encoders,
        }
    }

    pub fn with_defaults(data: &[u8], maximum_transmission_unit: u16) -> Encoder {
        let config = ObjectTransmissionInformation::with_defaults(
            data.len() as u64,
            maximum_transmission_unit,
        );

        Encoder::new(data, config)
    }

    pub fn get_config(&self) -> ObjectTransmissionInformation {
        self.config
    }

    pub fn get_encoded_packets(&self, repair_packets_per_block: u32) -> Vec<EncodingPacket> {
        let mut packets = vec![];
        for encoder in self.blocks.iter() {
            packets.extend(encoder.source_packets());
            packets.extend(encoder.repair_packets(0, repair_packets_per_block));
        }
        packets
    }

    pub fn get_block_encoders(&self) -> &Vec<SourceBlockEncoder> {
        &self.blocks
    }

    pub fn get_block_encoder_at(&self, index: usize) -> &SourceBlockEncoder {
        &self.blocks[index]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct SourceBlockEncodingPlan {
    operations: Vec<SymbolOps>,
    source_symbol_count: u16,
}

impl SourceBlockEncodingPlan {
    // Generates an encoding plan that is valid for any combination of data length and symbol size
    // where ceil(data_length / symbol_size) = symbol_count
    pub fn generate(symbol_count: u16) -> SourceBlockEncodingPlan {
        // TODO: refactor pi_solver, so that we don't need this dummy data to generate a plan
        let symbols = vec![Symbol::new(vec![0]); symbol_count as usize];
        let (_, ops) = gen_intermediate_symbols(&symbols, 1, SPARSE_MATRIX_THRESHOLD);
        SourceBlockEncodingPlan {
            operations: ops.unwrap(),
            source_symbol_count: symbol_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct SourceBlockEncoder {
    source_block_id: u8,
    source_symbols: Vec<Symbol>,
    intermediate_symbols: Vec<Symbol>,
}

impl SourceBlockEncoder {
    #[deprecated(
        since = "1.3.0",
        note = "Use the new2() function instead. In version 2.0, that function will replace this one"
    )]
    pub fn new(source_block_id: u8, symbol_size: u16, data: &[u8]) -> SourceBlockEncoder {
        let config = ObjectTransmissionInformation::new(0, symbol_size, 0, 1, 1);
        SourceBlockEncoder::new2(source_block_id, &config, data)
    }

    fn create_symbols(config: &ObjectTransmissionInformation, data: &[u8]) -> Vec<Symbol> {
        assert_eq!(data.len() % config.symbol_size() as usize, 0);
        if config.sub_blocks() > 1 {
            let mut symbols = vec![vec![]; data.len() / config.symbol_size() as usize];
            let (tl, ts, nl, ns) = partition(
                (config.symbol_size() / config.symbol_alignment() as u16) as u32,
                config.sub_blocks(),
            );
            // Divide the block into sub-blocks and then concatenate the sub-symbols into symbols
            // See second to last paragraph in section 4.4.1.2.
            let mut offset = 0;
            for sub_block in 0..(nl + ns) as u32 {
                let bytes = if sub_block < nl {
                    tl as usize * config.symbol_alignment() as usize
                } else {
                    ts as usize * config.symbol_alignment() as usize
                };
                for symbol in &mut symbols {
                    symbol.extend_from_slice(&data[offset..offset + bytes]);
                    offset += bytes;
                }
            }
            assert_eq!(offset, data.len());
            symbols.drain(..).map(Symbol::new).collect()
        } else {
            data.chunks(config.symbol_size() as usize)
                .map(|x| Symbol::new(Vec::from(x)))
                .collect()
        }
    }

    // TODO: rename this to new() in version 2.0
    pub fn new2(
        source_block_id: u8,
        config: &ObjectTransmissionInformation,
        data: &[u8],
    ) -> SourceBlockEncoder {
        let source_symbols = SourceBlockEncoder::create_symbols(config, data);

        let (intermediate_symbols, _) = gen_intermediate_symbols(
            &source_symbols,
            config.symbol_size() as usize,
            SPARSE_MATRIX_THRESHOLD,
        );

        SourceBlockEncoder {
            source_block_id,
            source_symbols,
            intermediate_symbols: intermediate_symbols.unwrap(),
        }
    }

    #[deprecated(
        since = "1.3.0",
        note = "Use the with_encoding_plan2() function instead. In version 2.0, that function will replace this one"
    )]
    pub fn with_encoding_plan(
        source_block_id: u8,
        symbol_size: u16,
        data: &[u8],
        plan: &SourceBlockEncodingPlan,
    ) -> SourceBlockEncoder {
        let config = ObjectTransmissionInformation::new(0, symbol_size, 0, 1, 1);
        SourceBlockEncoder::with_encoding_plan2(source_block_id, &config, data, plan)
    }

    // TODO: rename this to with_encoding_plan() in version 2.0
    pub fn with_encoding_plan2(
        source_block_id: u8,
        config: &ObjectTransmissionInformation,
        data: &[u8],
        plan: &SourceBlockEncodingPlan,
    ) -> SourceBlockEncoder {
        let source_symbols = SourceBlockEncoder::create_symbols(config, data);
        // TODO: this could be more lenient and support anything with the same extended symbol count
        assert_eq!(source_symbols.len(), plan.source_symbol_count as usize);

        let intermediate_symbols = gen_intermediate_symbols_with_plan(
            &source_symbols,
            config.symbol_size() as usize,
            &plan.operations,
        );

        SourceBlockEncoder {
            source_block_id,
            source_symbols,
            intermediate_symbols,
        }
    }

    pub fn source_packets(&self) -> Vec<EncodingPacket> {
        let mut esi: i32 = -1;
        self.source_symbols
            .iter()
            .map(|symbol| {
                esi += 1;
                EncodingPacket::new(
                    PayloadId::new(self.source_block_id, esi as u32),
                    symbol.as_bytes().to_vec(),
                )
            })
            .collect()
    }

    // See section 5.3.4
    pub fn repair_packets(&self, start_repair_symbol_id: u32, packets: u32) -> Vec<EncodingPacket> {
        let start_encoding_symbol_id = start_repair_symbol_id
            + extended_source_block_symbols(self.source_symbols.len() as u32);
        let mut result = vec![];
        let lt_symbols = num_lt_symbols(self.source_symbols.len() as u32);
        let sys_index = systematic_index(self.source_symbols.len() as u32);
        let p1 = calculate_p1(self.source_symbols.len() as u32);
        for i in 0..packets {
            let tuple = intermediate_tuple(start_encoding_symbol_id + i, lt_symbols, sys_index, p1);
            result.push(EncodingPacket::new(
                PayloadId::new(self.source_block_id, start_encoding_symbol_id + i),
                enc(
                    self.source_symbols.len() as u32,
                    &self.intermediate_symbols,
                    tuple,
                )
                .into_bytes(),
            ));
        }
        result
    }
}

#[allow(non_snake_case)]
fn create_d(
    source_block: &[Symbol],
    symbol_size: usize,
    extended_source_symbols: usize,
) -> Vec<Symbol> {
    let L = num_intermediate_symbols(source_block.len() as u32);
    let S = num_ldpc_symbols(source_block.len() as u32);
    let H = num_hdpc_symbols(source_block.len() as u32);

    let mut D = Vec::with_capacity(L as usize);
    for _ in 0..(S + H) {
        D.push(Symbol::zero(symbol_size));
    }
    for symbol in source_block {
        D.push(symbol.clone());
    }
    // Extend the source block with padding. See section 5.3.2
    for _ in 0..(extended_source_symbols as usize - source_block.len()) {
        D.push(Symbol::zero(symbol_size));
    }
    assert_eq!(D.len(), L as usize);
    D
}

// See section 5.3.3.4
#[allow(non_snake_case)]
fn gen_intermediate_symbols(
    source_block: &[Symbol],
    symbol_size: usize,
    sparse_threshold: u32,
) -> (Option<Vec<Symbol>>, Option<Vec<SymbolOps>>) {
    let extended_source_symbols = extended_source_block_symbols(source_block.len() as u32);
    let D = create_d(source_block, symbol_size, extended_source_symbols as usize);

    let indices: Vec<u32> = (0..extended_source_symbols).collect();
    if extended_source_symbols >= sparse_threshold {
        let (A, hdpc) =
            generate_constraint_matrix::<SparseBinaryMatrix>(extended_source_symbols, &indices);
        let(_, a, b) = fused_inverse_mul_symbols(A, hdpc, D, extended_source_symbols);
        return (a,b);
    } else {
        let (A, hdpc) =
            generate_constraint_matrix::<DenseBinaryMatrix>(extended_source_symbols, &indices);
        let(_, a, b) =  fused_inverse_mul_symbols(A, hdpc, D, extended_source_symbols);
        return (a,b);
    }
}

#[allow(non_snake_case)]
fn gen_intermediate_symbols_with_plan(
    source_block: &[Symbol],
    symbol_size: usize,
    operation_vector: &[SymbolOps],
) -> Vec<Symbol> {
    let extended_source_symbols = extended_source_block_symbols(source_block.len() as u32);
    let mut D = create_d(source_block, symbol_size, extended_source_symbols as usize);

    for op in operation_vector {
        perform_op(op, &mut D);
    }
    D
}

// Enc[] function, as defined in section 5.3.5.3
#[allow(clippy::many_single_char_names)]
fn enc(
    source_block_symbols: u32,
    intermediate_symbols: &[Symbol],
    source_tuple: (u32, u32, u32, u32, u32, u32),
) -> Symbol {
    let w = num_lt_symbols(source_block_symbols);
    let p = num_pi_symbols(source_block_symbols);
    let p1 = calculate_p1(source_block_symbols);
    let (d, a, mut b, d1, a1, mut b1) = source_tuple;

    assert!(1 <= a && a < w);
    assert!(b < w);
    assert!(d1 == 2 || d1 == 3);
    assert!(1 <= a1 && a < w);
    assert!(b1 < w);

    let mut result = intermediate_symbols[b as usize].clone();
    for _ in 1..d {
        b = (b + a) % w;
        result += &intermediate_symbols[b as usize];
    }

    while b1 >= p {
        b1 = (b1 + a1) % p1;
    }

    result += &intermediate_symbols[(w + b1) as usize];

    for _ in 1..d1 {
        b1 = (b1 + a1) % p1;
        while b1 >= p {
            b1 = (b1 + a1) % p1;
        }
        result += &intermediate_symbols[(w + b1) as usize];
    }

    result
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use crate::base::intermediate_tuple;
    use crate::encoder::enc;
    use crate::encoder::gen_intermediate_symbols;
    use crate::symbol::Symbol;
    use crate::systematic_constants::num_lt_symbols;
    use crate::systematic_constants::num_pi_symbols;
    use crate::systematic_constants::{
        calculate_p1, num_ldpc_symbols, systematic_index, MAX_SOURCE_SYMBOLS_PER_BLOCK,
    };
    use crate::{Encoder, EncoderBuilder, EncodingPacket, ObjectTransmissionInformation};
    use std::collections::HashSet;

    const SYMBOL_SIZE: usize = 4;
    const NUM_SYMBOLS: u32 = 100;

    fn gen_test_data(size: usize) -> Vec<u8> {
        let mut data: Vec<u8> = vec![0; size];
        for i in 0..size {
            data[i] = rand::thread_rng().gen();
        }
        data
    }

    fn gen_test_symbols() -> Vec<Symbol> {
        let mut source_block: Vec<Symbol> = vec![];
        for _ in 0..NUM_SYMBOLS {
            let data = gen_test_data(SYMBOL_SIZE);
            source_block.push(Symbol::new(data));
        }
        source_block
    }

    #[test]
    fn enc_constraint_dense() {
        enc_constraint(MAX_SOURCE_SYMBOLS_PER_BLOCK + 1);
    }

    #[test]
    fn enc_constraint_sparse() {
        enc_constraint(0);
    }

    fn enc_constraint(sparse_threshold: u32) {
        let source_symbols = gen_test_symbols();

        let (intermediate_symbols, _) =
            gen_intermediate_symbols(&source_symbols, SYMBOL_SIZE, sparse_threshold);
        let intermediate_symbols = intermediate_symbols.unwrap();

        let lt_symbols = num_lt_symbols(NUM_SYMBOLS);
        let sys_index = systematic_index(NUM_SYMBOLS);
        let p1 = calculate_p1(NUM_SYMBOLS);
        // See section 5.3.3.4.1, item 1.
        for i in 0..source_symbols.len() {
            let tuple = intermediate_tuple(i as u32, lt_symbols, sys_index, p1);
            let encoded = enc(NUM_SYMBOLS, &intermediate_symbols, tuple);
            assert_eq!(source_symbols[i], encoded);
        }
    }

    #[test]
    fn ldpc_constraint_dense() {
        ldpc_constraint(MAX_SOURCE_SYMBOLS_PER_BLOCK + 1);
    }

    #[test]
    fn ldpc_constraint_sparse() {
        ldpc_constraint(0);
    }

    #[allow(non_snake_case)]
    fn ldpc_constraint(sparse_threshold: u32) {
        let (intermediate_symbols, _) =
            gen_intermediate_symbols(&gen_test_symbols(), SYMBOL_SIZE, sparse_threshold);
        let C = intermediate_symbols.unwrap();
        let S = num_ldpc_symbols(NUM_SYMBOLS) as usize;
        let P = num_pi_symbols(NUM_SYMBOLS) as usize;
        let W = num_lt_symbols(NUM_SYMBOLS) as usize;
        let B = W - S;

        // See section 5.3.3.3
        let mut D = vec![];
        for i in 0..S {
            D.push(C[B + i].clone());
        }

        for i in 0..B {
            let a = 1 + i / S;
            let b = i % S;
            D[b] += &C[i];

            let b = (b + a) % S;
            D[b] += &C[i];

            let b = (b + a) % S;
            D[b] += &C[i];
        }

        for i in 0..S {
            let a = i % P;
            let b = (i + 1) % P;
            D[i] += &C[W + a];
            D[i] += &C[W + b];
        }

        for i in 0..S {
            assert_eq!(Symbol::zero(SYMBOL_SIZE), D[i]);
        }
    }

    #[test]
    fn test_builder() {
        let data = vec![0, 1, 2, 3];
        let encoder = Encoder::with_defaults(&data, 1024);
        let mut builder = EncoderBuilder::new();
        builder.set_max_packet_size(1024);
        assert_eq!(builder.build(&data), encoder);
    }

    #[test]
    fn padding_constraint_exact() {
        let packet_size: u16 = 1024;
        let padding_size: usize = 0;
        let data_size: usize = packet_size as usize * 2 - padding_size;
        padding_constraint(packet_size, padding_size, data_size);
    }

    #[test]
    fn padding_constraint_42_bytes() {
        let packet_size: u16 = 1024;
        let padding_size: usize = 42;
        let data_size: usize = packet_size as usize * 2 - padding_size;
        padding_constraint(packet_size, padding_size, data_size);
    }

    fn padding_constraint(packet_size: u16, padding_size: usize, data_size: usize) {
        let data = gen_test_data(data_size);
        let encoder = Encoder::with_defaults(&data, packet_size);

        fn accumulate_data(acc: Vec<u8>, packet: EncodingPacket) -> Vec<u8> {
            let mut updated_acc = acc.clone();
            updated_acc.extend_from_slice(packet.data());
            updated_acc
        }

        let padded_data = encoder
            .get_block_encoders()
            .iter()
            .flat_map(|block| block.source_packets())
            .fold(vec![], accumulate_data);

        assert_eq!(data_size + padding_size, padded_data.len());
        assert_eq!(data[..], padded_data[..data_size]);
    }

    #[test]
    fn unique_blocks() {
        let data = gen_test_data(120);
        let config = ObjectTransmissionInformation::new(120, 10, 10, 0, 2);
        let encoder = Encoder::new(&data, config);
        assert!(encoder.get_block_encoders().len() > 1);
        let mut ids = HashSet::new();
        for block in encoder.get_block_encoders().iter() {
            ids.insert(block.source_block_id);
        }
        assert_eq!(ids.len(), encoder.get_block_encoders().len());
    }
}
