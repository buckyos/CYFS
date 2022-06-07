# eciesrs

[![Codacy Badge](https://api.codacy.com/project/badge/Grade/1c6d6ed949dd4836ab97421039e8be75)](https://www.codacy.com/app/ecies/rs)
[![License](https://img.shields.io/github/license/ecies/rs.svg)](https://github.com/ecies/rs)
[![Circle CI](https://img.shields.io/circleci/project/ecies/rs/master.svg)](https://circleci.com/gh/ecies/rs)
[![Crates](https://img.shields.io/crates/v/ecies)](https://crates.io/crates/ecies)

Elliptic Curve Integrated Encryption Scheme for secp256k1 in Rust, based on [pure Rust implementation](https://github.com/paritytech/libsecp256k1) of secp256k1.

This is the Rust version of [eciespy](https://github.com/ecies/py).

## API

```rust
pub fn encrypt(receiver_pub: &[u8], msg: &[u8]) -> Result<Vec<u8>, SecpError>
```

```rust
pub fn decrypt(receiver_sec: &[u8], msg: &[u8]) -> Result<Vec<u8>, SecpError>
```

## Quick Start

```rust
const MSG: &str = "helloworld";
let (sk, pk) = generate_keypair();
let (sk, pk) = (&sk.serialize(), &pk.serialize());

let msg = MSG.as_bytes();
assert_eq!(
    msg,
    decrypt(sk, &encrypt(pk, msg).unwrap()).unwrap().as_slice()
);
```

## Release Notes

### 0.1.1 ~ 0.1.4

- Bump dependencies
- Update documentation
- Fix error handling

### 0.1.0

- First beta version release
