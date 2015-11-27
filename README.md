# Rust Modbus
[![Build Status](https://travis-ci.org/hirschenberger/modbus-rs.svg)](https://travis-ci.org/hirschenberger/modbus-rs)
[![Coverage Status](https://coveralls.io/repos/hirschenberger/modbus-rs/badge.svg?branch=master&service=github)](https://coveralls.io/github/hirschenberger/modbus-rs?branch=master)
[![](http://meritbadge.herokuapp.com/modbus)](https://crates.io/crates/modbus)
[![License](http://img.shields.io/:license-MIT-blue.svg)](http://doge.mit-license.org)


Modbus implementation in pure Rust.

## Usage
Add `modbus` to your `Cargo.toml` dependencies:

```toml
[dependencies]
modbus = "0.1.0"
```

Import the `modbus` crate and use it's functions:

```rust
use modbus::{Client, BitValue};
use modbus::tcp;

let mut client = tcp::Ctx::new("192.168.0.10");

client.write_single_coil(1, BitValue::On).unwrap();
client.write_single_coil(3, BitValue::On).unwrap();

let res = client.read_coils(0, 5).unwrap();

// res ==  vec![BitValue::Off, BitValue::On, BitValue::Off, BitValue::On, BitValue::Off]);
```
See the [documentation](http://hirschenberger.github.io/modbus-rs/modbus/index.html) for usage examples and further reference.


## License
Copyright © 2015 Falco Hirschenberger

Distributed under the [MIT License](LICENSE).
