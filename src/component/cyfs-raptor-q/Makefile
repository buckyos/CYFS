build: pre
	cargo build

pre:
	cargo deny check licenses
	cargo fmt --all -- --check
	cargo clippy --all

release: pre
	cargo build --release

test: pre
	cargo build --all-features
	cargo test --features benchmarking

test_extended: pre
	RUSTFLAGS="-C opt-level=3" nice cargo test --features benchmarking -- --ignored --nocapture

bench: pre
	cargo bench --features benchmarking

profile:
	RUSTFLAGS='-Cforce-frame-pointers' cargo bench --no-run --features benchmarking

build_py: pre
	maturin build --cargo-extra-args="--features python"

release_py: pre
	maturin build --release --cargo-extra-args="--features python"

publish_py: test_py
	docker pull quay.io/pypa/manylinux2014_x86_64
	docker run -it --rm -v $(shell pwd):/raptorq quay.io/pypa/manylinux2014_x86_64 /raptorq/py_publish.sh

install_py: pre
	maturin develop --cargo-extra-args="--features python"

test_py: install_py
	python3 -m unittest discover
