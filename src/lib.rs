//! Modbus implementation in pure Rust.
//!
//! # Examples
//!
//! ```
//! # extern crate modbus;
//! # extern crate test_server;
//! # use test_server::start_dummy_server;
//! # fn main() {
//! use modbus::{Client, Coil};
//! use modbus::tcp;
//! # if cfg!(feature = "modbus-server-tests") {
//! # let (_s, port) = start_dummy_server();
//!
//! // let port = 502;
//! let mut client = tcp::Transport::new_with_port("127.0.0.1", port).unwrap();
//! assert!(client.write_single_coil(0, Coil::On).is_ok());
//! # }
//! # }
//! ```

#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#[macro_use]
extern crate enum_primitive;
extern crate num;
extern crate rustc_serialize;
extern crate bincode;
extern crate byteorder;

use std::io;
use bincode::rustc_serialize::{DecodingError, EncodingError};

mod binary;
mod client;
pub mod scoped;

/// The Modbus TCP backend implements a Modbus variant used for communication over TCP/IPv4 networks.
pub mod tcp;
pub use tcp::{Transport};
pub use client::{Client};

type Address  = u16;
type Quantity = u16;
type Value    = u16;

enum Function<'a> {
    ReadCoils(Address, Quantity),
    ReadDiscreteInputs(Address, Quantity),
    ReadHoldingRegisters(Address, Quantity),
    ReadInputRegisters(Address, Quantity),
    WriteSingleCoil(Address, Value),
    WriteSingleRegister(Address, Value),
    WriteMultipleCoils(Address, Quantity, &'a[u8]),
    WriteMultipleRegisters(Address, Quantity, &'a[u8])
}

impl<'a> Function<'a> {
    fn code(&self) -> u8 {
        match *self {
            Function::ReadCoils(_, _)                 => 0x01,
            Function::ReadDiscreteInputs(_, _)        => 0x02,
            Function::ReadHoldingRegisters(_, _)      => 0x03,
            Function::ReadInputRegisters(_, _)        => 0x04,
            Function::WriteSingleCoil(_, _)           => 0x05,
            Function::WriteSingleRegister(_, _)       => 0x06,
            Function::WriteMultipleCoils(_, _, _)     => 0x0f,
            Function::WriteMultipleRegisters(_, _, _) => 0x10
        }
    //
    // ReadExceptionStatus     = 0x07,
    // ReportSlaveId           = 0x11,
    // MaskWriteRegister       = 0x16,
    // WriteAndReadRegisters   = 0x17
    }
}

enum_from_primitive! {
#[derive(Debug, PartialEq)]
/// Modbus exception codes returned from the server.
pub enum ExceptionCode {
    IllegalFunction         = 0x01,
    IllegalDataAddress      = 0x02,
    IllegalDataValue        = 0x03,
    SlaveOrServerFailure    = 0x04,
    Acknowledge             = 0x05,
    SlaveOrServerBusy       = 0x06,
    NegativeAcknowledge     = 0x07,
    MemoryParity            = 0x08,
    NotDefined              = 0x09,
    GatewayPath             = 0x0a,
    GatewayTarget           = 0x0b
}
}

/// Combination of Modbus, IO and data corruption errors
#[derive(Debug)]
pub enum Error {
    Exception(ExceptionCode),
    Io(io::Error),
    InvalidResponse,
    InvalidData
}

impl From<ExceptionCode> for Error {
    fn from(err: ExceptionCode) -> Error {
        Error::Exception(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<DecodingError> for Error {
    fn from(_err: DecodingError) -> Error {
        Error::InvalidData
    }
}

impl From<EncodingError> for Error {
    fn from(_err: EncodingError) -> Error {
        Error::InvalidData
    }
}

impl From<byteorder::Error> for Error {
    fn from(_err: byteorder::Error) -> Error {
        Error::InvalidData
    }
}

/// Result type used to nofify success or failure in communication
pub type Result<T> = std::result::Result<T, Error>;


/// Single bit status values, used in read or write coil functions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Coil {
    On,
    Off
}

impl Coil {
    fn code(&self) -> u16 {
        match *self {
            Coil::On  => 0xff00,
            Coil::Off => 0x0000
        }
    }
}
