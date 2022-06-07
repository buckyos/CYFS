# raptorq
[![Build Status](https://travis-ci.com/cberner/raptorq.svg?branch=master)](https://travis-ci.com/cberner/raptorq)
[![Crates](https://img.shields.io/crates/v/raptorq.svg)](https://crates.io/crates/raptorq)
[![Documentation](https://docs.rs/raptorq/badge.svg)](https://docs.rs/raptorq)
[![PyPI](https://img.shields.io/pypi/v/raptorq.svg)](https://pypi.org/project/raptorq/)
[![dependency status](https://deps.rs/repo/github/cberner/raptorq/status.svg)](https://deps.rs/repo/github/cberner/raptorq)

Rust implementation of RaptorQ (RFC6330)

Recovery properties:
Reconstruction probability after receiving K + h packets = 1 - 1/256^(h + 1). Where K is the number of packets in the
original message, and h is the number of additional packets received.
See "RaptorQ Technical Overview" by Qualcomm

This crate requires Rust 1.46 or newer.

### Examples
See the `examples/` directory for usage.

### Benchmarks

The following were run on an Intel Core i5-6600K @ 3.50GHz

```
Symbol size: 1280 bytes (without pre-built plan)
symbol count = 10, encoded 127 MB in 0.532secs, throughput: 1924.7Mbit/s
symbol count = 100, encoded 127 MB in 0.590secs, throughput: 1734.6Mbit/s
symbol count = 250, encoded 127 MB in 0.572secs, throughput: 1788.4Mbit/s
symbol count = 500, encoded 127 MB in 0.549secs, throughput: 1858.8Mbit/s
symbol count = 1000, encoded 126 MB in 0.599secs, throughput: 1695.5Mbit/s
symbol count = 2000, encoded 126 MB in 0.673secs, throughput: 1509.1Mbit/s
symbol count = 5000, encoded 122 MB in 0.758secs, throughput: 1288.3Mbit/s
symbol count = 10000, encoded 122 MB in 0.953secs, throughput: 1024.7Mbit/s
symbol count = 20000, encoded 122 MB in 1.383secs, throughput: 706.1Mbit/s
symbol count = 50000, encoded 122 MB in 2.041secs, throughput: 478.5Mbit/s

Symbol size: 1280 bytes (with pre-built plan)
symbol count = 10, encoded 127 MB in 0.241secs, throughput: 4248.7Mbit/s
symbol count = 100, encoded 127 MB in 0.160secs, throughput: 6396.5Mbit/s
symbol count = 250, encoded 127 MB in 0.173secs, throughput: 5913.0Mbit/s
symbol count = 500, encoded 127 MB in 0.176secs, throughput: 5798.3Mbit/s
symbol count = 1000, encoded 126 MB in 0.200secs, throughput: 5078.1Mbit/s
symbol count = 2000, encoded 126 MB in 0.208secs, throughput: 4882.8Mbit/s
symbol count = 5000, encoded 122 MB in 0.280secs, throughput: 3487.7Mbit/s
symbol count = 10000, encoded 122 MB in 0.400secs, throughput: 2441.4Mbit/s
symbol count = 20000, encoded 122 MB in 0.494secs, throughput: 1976.8Mbit/s
symbol count = 50000, encoded 122 MB in 0.656secs, throughput: 1488.7Mbit/s

Symbol size: 1280 bytes
symbol count = 10, decoded 127 MB in 0.723secs using 0.0% overhead, throughput: 1416.2Mbit/s
symbol count = 100, decoded 127 MB in 0.701secs using 0.0% overhead, throughput: 1460.0Mbit/s
symbol count = 250, decoded 127 MB in 0.650secs using 0.0% overhead, throughput: 1573.8Mbit/s
symbol count = 500, decoded 127 MB in 0.638secs using 0.0% overhead, throughput: 1599.5Mbit/s
symbol count = 1000, decoded 126 MB in 0.676secs using 0.0% overhead, throughput: 1502.4Mbit/s
symbol count = 2000, decoded 126 MB in 0.764secs using 0.0% overhead, throughput: 1329.4Mbit/s
symbol count = 5000, decoded 122 MB in 0.896secs using 0.0% overhead, throughput: 1089.9Mbit/s
symbol count = 10000, decoded 122 MB in 1.176secs using 0.0% overhead, throughput: 830.4Mbit/s
symbol count = 20000, decoded 122 MB in 1.489secs using 0.0% overhead, throughput: 655.9Mbit/s
symbol count = 50000, decoded 122 MB in 2.633secs using 0.0% overhead, throughput: 370.9Mbit/s

symbol count = 10, decoded 127 MB in 0.713secs using 5.0% overhead, throughput: 1436.1Mbit/s
symbol count = 100, decoded 127 MB in 0.702secs using 5.0% overhead, throughput: 1457.9Mbit/s
symbol count = 250, decoded 127 MB in 0.637secs using 5.0% overhead, throughput: 1605.9Mbit/s
symbol count = 500, decoded 127 MB in 0.613secs using 5.0% overhead, throughput: 1664.8Mbit/s
symbol count = 1000, decoded 126 MB in 0.643secs using 5.0% overhead, throughput: 1579.5Mbit/s
symbol count = 2000, decoded 126 MB in 0.701secs using 5.0% overhead, throughput: 1448.8Mbit/s
symbol count = 5000, decoded 122 MB in 0.826secs using 5.0% overhead, throughput: 1182.3Mbit/s
symbol count = 10000, decoded 122 MB in 1.061secs using 5.0% overhead, throughput: 920.4Mbit/s
symbol count = 20000, decoded 122 MB in 1.380secs using 5.0% overhead, throughput: 707.7Mbit/s
symbol count = 50000, decoded 122 MB in 2.341secs using 5.0% overhead, throughput: 417.2Mbit/s
```

### Public API
Note that the additional classes exported by the `benchmarking` feature flag are not considered part of this
crate's public API. Breaking changes to those classes may occur without warning. The flag is only provided
so that internal classes can be used in this crate's benchmarks.

## Python bindings

The Python bindings are generated using [pyo3](https://github.com/PyO3/pyo3). 

Some operating systems require additional packages to be installed.
```
$ sudo apt install python3-dev
```

[maturin](https://github.com/PyO3/maturin) is recommended for building the Python bindings in this crate.
```
$ pip install maturin
$ maturin build --cargo-extra-args="--features python"
```

Alternatively, refer to the [Building and Distribution section](https://pyo3.rs/v0.8.5/building_and_distribution.html) in the [pyo3 user guide](https://pyo3.rs/v0.8.5/).
Note, you must pass the `--cargo-extra-args="--features python"` argument to Maturin when building this crate
to enable the Python binding features.

## License

Licensed under

 * Apache License, Version 2.0 ([LICENSE](LICENSE) or http://www.apache.org/licenses/LICENSE-2.0)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be licensed as above, without any
additional terms or conditions.
