//! A set of objects which automatically change their register or coil value when they go out of scope
//!
//! # Examples
//!
//! When the `auto` object goes out of scope and is dropped, the value of coil `10` is switched `On`:
//!
//! ```
//! # extern crate modbus;
//! # extern crate test_server;
//! # use test_server::start_dummy_server;
//! # fn main() {
//! use modbus::{Client, Coil};
//! use modbus::tcp;
//! use modbus::scoped::{ScopedCoil, CoilDropFunction};
//! # if cfg!(feature = "modbus-server-tests") {
//! # let (_s, port) = start_dummy_server(Some(22222));
//!
//! // let port = 502;
//! let mut client = tcp::Transport::new_with_port("127.0.0.1", port).unwrap();
//! {
//!    let mut auto = ScopedCoil::new(&mut client, 10, CoilDropFunction::On).unwrap();
//!    assert_eq!(auto.mut_transport().read_coils(10, 1).unwrap(), vec![Coil::Off]);
//! }
//! assert_eq!(client.read_coils(10, 1).unwrap(), vec![Coil::On]);
//! # }
//! # }
//! ```
//!
//! When the `auto` object goes out of scope and is dropped, the value of register `10` is modified by
//! function `fun`:
//!
//! ```
//! # extern crate modbus;
//! # extern crate test_server;
//! # use test_server::start_dummy_server;
//! # fn main() {
//! use modbus::{Client, Coil};
//! use modbus::tcp;
//! use modbus::scoped::{ScopedRegister, RegisterDropFunction};
//! # if cfg!(feature = "modbus-server-tests") {
//! # let (_s, port) = start_dummy_server(Some(22223));
//!
//! // let port = 502;
//! let mut client = tcp::Transport::new_with_port("127.0.0.1", port).unwrap();
//! client.write_single_register(10, 1);
//! {
//!     let fun = |v| v + 5;
//!     let mut auto = ScopedRegister::new(&mut client, 10, RegisterDropFunction::Fun(&fun)).unwrap();
//!     assert_eq!(auto.mut_transport().read_holding_registers(10, 1).unwrap(), vec![1]);
//! }
//! assert_eq!(client.read_holding_registers(10, 1).unwrap(), vec![6]);
//! # }
//! # }
//! ```

use {Coil, Transport, Client, Result};

/// Action to perform when the `ScopedCoil` is dropped.
pub enum CoilDropFunction {
    /// Set the coil to `Coil::On`
    On,
    /// Set the coil to `Coil::Off`
    Off,
    /// Toggle the current value.
    Toggle,
}

/// Action to perform when the `ScopedRegister` is dropped.
pub enum RegisterDropFunction<'a> {
    /// Set the register to zero value
    Zero,
    /// Increment the current register value by 1
    Increment,
    /// Decrement the current register value by 1
    Decrement,
    /// Set the register value to the given value.
    Value(u16),
    /// Execute the given function on the current value, setting the register with the result value.
    Fun(&'a Fn(u16) -> u16),
}

/// Auto object which modifies it's coil value depending on a given modification function if it
/// goes out of scope.
pub struct ScopedCoil<'a> {
    address: u16,
    fun: CoilDropFunction,
    transport: &'a mut Transport,
}

impl<'a> Drop for ScopedCoil<'a> {
    fn drop(&mut self) {
        let _ = self.transport
            .read_coils(self.address, 1)
            .and_then(|value| match value.len() {
                1 => {
                    let drop_value = match self.fun {
                        CoilDropFunction::On => Coil::On,
                        CoilDropFunction::Off => Coil::Off,
                        CoilDropFunction::Toggle => {
                            match value[0] {
                                Coil::On => Coil::Off,
                                Coil::Off => Coil::On,
                            }
                        }
                    };
                    let _ = self.transport.write_single_coil(self.address, drop_value);
                    Ok(())
                }
                _ => Ok(()),
            });
    }
}

impl<'a> ScopedCoil<'a> {
    /// Create a new `ScopedCoil` object with `address` and drop function when the object goes
    /// out of scope.
    pub fn new(transport: &mut Transport,
               address: u16,
               fun: CoilDropFunction)
               -> Result<ScopedCoil> {
        Ok(ScopedCoil {
            address: address,
            fun: fun,
            transport: transport,
        })
    }

    pub fn mut_transport(&mut self) -> &mut Transport {
        self.transport
    }
}

/// Auto object which modifies it's register value depending on a given modification function if it
/// goes out of scope.
pub struct ScopedRegister<'a> {
    address: u16,
    fun: RegisterDropFunction<'a>,
    transport: &'a mut Transport,
}

impl<'a> Drop for ScopedRegister<'a> {
    fn drop(&mut self) {
        let _ = self.transport
            .read_holding_registers(self.address, 1)
            .and_then(|value| match value.len() {
                1 => {
                    let drop_value = match self.fun {
                        RegisterDropFunction::Zero => 0u16,
                        RegisterDropFunction::Increment => value[0] + 1,
                        RegisterDropFunction::Decrement => value[0] - 1,
                        RegisterDropFunction::Value(v) => v,
                        RegisterDropFunction::Fun(f) => f(value[0]),
                    };
                    let _ = self.transport.write_single_register(self.address, drop_value);
                    Ok(())
                }
                _ => Ok(()),
            });
    }
}

impl<'a> ScopedRegister<'a> {
    /// Create a new `ScopedRegister` object with `address` and drop function when the object goes
    /// out of scope.
    pub fn new<'b>(transport: &'b mut Transport,
                   address: u16,
                   fun: RegisterDropFunction<'b>)
                   -> Result<ScopedRegister<'b>> {
        Ok(ScopedRegister {
            address: address,
            fun: fun,
            transport: transport,
        })
    }

    pub fn mut_transport(&mut self) -> &mut Transport {
        self.transport
    }
}
