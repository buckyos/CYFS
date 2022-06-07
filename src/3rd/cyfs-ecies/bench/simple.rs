use core::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use cyfs_ecies::{decrypt, encrypt, utils::generate_keypair};

const BIG_MSG_SIZE: usize = 100 * 1024 * 1024;
const BIG_MSG: [u8; BIG_MSG_SIZE] = [1u8; BIG_MSG_SIZE];

fn criterion_benchmark(c: &mut Criterion) {
    let (sk, pk) = generate_keypair();
    let (sk, pk) = (&sk.serialize(), &pk.serialize());

    let msg = &BIG_MSG;
    let encrypted = &encrypt(pk, msg).unwrap();

    c.bench_function("encrypt 100M", |b| b.iter(|| encrypt(pk, msg).unwrap()));

    c.bench_function("decrypt 100M", |b| b.iter(|| decrypt(sk, encrypted).unwrap()));
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).measurement_time(Duration::new(20, 0));
    targets = criterion_benchmark
}
criterion_main!(benches);
