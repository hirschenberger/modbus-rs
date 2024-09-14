# Rust Modbus
![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/hirschenberger/modbus/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/modbus)](https://crates.io/crates/modbus)
[![docs.rs](https://img.shields.io/docsrs/modbus)](https://docs.rs/modbus/latest/modbus)
[![Crates.io](https://img.shields.io/crates/d/modbus)](https://crates.io/crates/modbus)
[![License](http://img.shields.io/:license-MIT-blue.svg)](http://doge.mit-license.org)


Modbus implementation in pure Rust.

## Usage
Add `modbus` to your `Cargo.toml` dependencies:

```toml
[dependencies]
modbus = "1.1"
```

Import the `modbus` crate and use it's functions:

```rust
use modbus::{Client, Coil};
use modbus::tcp;

let mut client = tcp::Transport::new("192.168.0.10");

client.write_single_coil(1, Coil::On).unwrap();
client.write_single_coil(3, Coil::On).unwrap();

let res = client.read_coils(0, 5).unwrap();

// res ==  vec![Coil::Off, Coil::On, Coil::Off, Coil::On, Coil::Off];
```
See the [documentation](http://hirschenberger.github.io/modbus-rs/modbus/index.html) for usage examples and further reference and
the [examples](https://github.com/hirschenberger/modbus-rs/tree/master/examples) directory for a commandline client application.


## License
Copyright © 2015-2024 Falco Hirschenberger

Distributed under the [MIT License](LICENSE).
