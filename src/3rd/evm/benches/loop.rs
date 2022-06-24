use std::{str::FromStr, collections::BTreeMap};
use criterion::{criterion_group, criterion_main, Criterion};
use primitive_types::{U256, H160};
use evm::Config;
use evm::executor::{StackExecutor, MemoryStackState, StackSubstateMetadata};
use evm::backend::{MemoryAccount, MemoryVicinity, MemoryBackend};

fn run_loop_contract() {
	let config = Config::istanbul();

	let vicinity = MemoryVicinity {
		gas_price: U256::zero(),
		origin: H160::default(),
		block_hashes: Vec::new(),
		block_number: Default::default(),
		block_coinbase: Default::default(),
		block_timestamp: Default::default(),
		block_difficulty: Default::default(),
		block_gas_limit: Default::default(),
		chain_id: U256::one(),
	};

	let mut state = BTreeMap::new();
	state.insert(
		H160::from_str("0x1000000000000000000000000000000000000000").unwrap(),
		MemoryAccount {
			nonce: U256::one(),
			balance: U256::from(10000000),
			storage: BTreeMap::new(),
			code: hex::decode("6080604052348015600f57600080fd5b506004361060285760003560e01c80630f14a40614602d575b600080fd5b605660048036036020811015604157600080fd5b8101908080359060200190929190505050606c565b6040518082815260200191505060405180910390f35b6000806000905060005b83811015608f5760018201915080806001019150506076565b508091505091905056fea26469706673582212202bc9ec597249a9700278fe4ce78da83273cb236e76d4d6797b441454784f901d64736f6c63430007040033").unwrap(),
		}
	);
	state.insert(
		H160::from_str("0xf000000000000000000000000000000000000000").unwrap(),
		MemoryAccount {
			nonce: U256::one(),
			balance: U256::from(10000000),
			storage: BTreeMap::new(),
			code: Vec::new(),
		},
	);

	let backend = MemoryBackend::new(&vicinity, state);
	let metadata = StackSubstateMetadata::new(u64::max_value(), &config);
	let state = MemoryStackState::new(metadata, &backend);
	let mut executor = StackExecutor::new(state, &config);

	let _reason = executor.transact_call(
		H160::from_str("0xf000000000000000000000000000000000000000").unwrap(),
		H160::from_str("0x1000000000000000000000000000000000000000").unwrap(),
		U256::zero(),
		hex::decode("0f14a4060000000000000000000000000000000000000000000000000000000000b71b00").unwrap(),
		// hex::decode("0f14a4060000000000000000000000000000000000000000000000000000000000002ee0").unwrap(),
		u64::max_value(),
	);
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("loop contract", |b| b.iter(|| run_loop_contract()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
