# Omango-Sync

This crate provides Rust concurrency utilities 
in addition to the ones provided by the language and "std::sync".<br />

- [WaitGroup](src/wg.rs): support `Golang's WaitGroup`.
- [SingleFlight](src/single/flight.rs): provide multiplexing for workers have the same work.
- [SingleSource](src/single/source.rs): provide mechanism to synthesize data from multi sources.

## Table of Contents

- [Usage](#usage)
- [Compatibility](#compatibility)
- [License](#license)

## Usage

Add this to your `Cargo.toml`:
```toml
[dependencies]
omango-sync = "0.1.0"
```

## Compatibility

The minimum supported Rust version is `1.57`.

## License

The crate is licensed under the terms of the MIT
license. See [LICENSE](LICENSE) for more information.
