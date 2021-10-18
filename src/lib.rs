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
//! # let (_s, port) = start_dummy_server(Some(22221));
//!
//! let mut cfg = tcp::Config::default();
//! # cfg.tcp_port = port;
//! let mut client = tcp::Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
//! assert!(client.write_single_coil(0, Coil::On).is_ok());
//! # }
//! # }
//! ```

#[macro_use]
extern crate enum_primitive;
extern crate byteorder;

use std::fmt;
use std::io;
use std::str::FromStr;

pub mod binary;
mod client;

pub mod scoped;

/// The Modbus TCP backend implements a Modbus variant used for communication over TCP/IPv4 networks.
pub mod tcp;
pub use client::Client;
pub use tcp::Config;
pub use tcp::Transport;

type Address = u16;
type Quantity = u16;
type Value = u16;

enum Function<'a> {
    ReadCoils(Address, Quantity),
    ReadDiscreteInputs(Address, Quantity),
    ReadHoldingRegisters(Address, Quantity),
    ReadInputRegisters(Address, Quantity),
    WriteSingleCoil(Address, Value),
    WriteSingleRegister(Address, Value),
    WriteMultipleCoils(Address, Quantity, &'a [u8]),
    WriteMultipleRegisters(Address, Quantity, &'a [u8]),
}

impl<'a> Function<'a> {
    fn code(&self) -> u8 {
        match *self {
            Function::ReadCoils(_, _) => 0x01,
            Function::ReadDiscreteInputs(_, _) => 0x02,
            Function::ReadHoldingRegisters(_, _) => 0x03,
            Function::ReadInputRegisters(_, _) => 0x04,
            Function::WriteSingleCoil(_, _) => 0x05,
            Function::WriteSingleRegister(_, _) => 0x06,
            Function::WriteMultipleCoils(_, _, _) => 0x0f,
            Function::WriteMultipleRegisters(_, _, _) => 0x10,
        }
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

/// `InvalidData` reasons
#[derive(Debug)]
pub enum Reason {
    UnexpectedReplySize,
    BytecountNotEven,
    SendBufferEmpty,
    RecvBufferEmpty,
    SendBufferTooBig,
    DecodingError,
    EncodingError,
    InvalidByteorder,
    Custom(String),
}

/// Combination of Modbus, IO and data corruption errors
#[derive(Debug)]
pub enum Error {
    Exception(ExceptionCode),
    Io(io::Error),
    InvalidResponse,
    InvalidData(Reason),
    InvalidFunction,
    ParseCoilError,
    ParseInfoError,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Error::*;

        match *self {
            Exception(ref code) => write!(f, "modbus exception: {:?}", code),
            Io(ref err) => write!(f, "I/O error: {}", err),
            InvalidResponse => write!(f, "invalid response"),
            InvalidData(ref reason) => write!(f, "invalid data: {:?}", reason),
            InvalidFunction => write!(f, "invalid modbus function"),
            ParseCoilError => write!(f, "parse coil could not be parsed"),
            ParseInfoError => write!(f, "failed parsing device info as utf8"),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        use Error::*;

        match *self {
            Exception(_) => "modbus exception",
            Io(_) => "I/O error",
            InvalidResponse => "invalid response",
            InvalidData(_) => "invalid data",
            InvalidFunction => "invalid modbus function",
            ParseCoilError => "parse coil could not be parsed",
            ParseInfoError => "failed parsing device info as utf8",
        }
    }

    fn cause(&self) -> Option<&dyn std::error::Error> {
        match *self {
            Error::Io(ref err) => Some(err),
            _ => None,
        }
    }
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

/// Result type used to nofify success or failure in communication
pub type Result<T> = std::result::Result<T, Error>;

/// Single bit status values, used in read or write coil functions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Coil {
    On,
    Off,
}

impl Coil {
    fn code(self) -> u16 {
        match self {
            Coil::On => 0xff00,
            Coil::Off => 0x0000,
        }
    }
}

impl FromStr for Coil {
    type Err = Error;
    fn from_str(s: &str) -> Result<Coil> {
        if s == "On" {
            Ok(Coil::On)
        } else if s == "Off" {
            Ok(Coil::Off)
        } else {
            Err(Error::ParseCoilError)
        }
    }
}

impl From<bool> for Coil {
    fn from(b: bool) -> Coil {
        if b {
            Coil::On
        } else {
            Coil::Off
        }
    }
}

impl std::ops::Not for Coil {
    type Output = Coil;

    fn not(self) -> Coil {
        match self {
            Coil::On => Coil::Off,
            Coil::Off => Coil::On,
        }
    }
}

#[cfg(feature = "read-device-info")]
/// Types specific to the special ReadDeviceInfo function
pub mod mei {
    /**
     * Describes object standard conformity
     *
     * - **Basic** - Mandatory for Modbus standard conformity
     * - **Regular** - Defined in the standard, but implementation is optional
     * - **Extended** - Optional fields that are reserved for device specific information
     */
    #[derive(Copy, Clone, Debug)]
    pub enum DeviceInfoCategory {
        Basic,
        Regular,
        Extended,
    }

    /**
     * Struct representing a device information object.
     *
     * The following object IDs are defined in the Modbus standard:
     * - **0x00** *BASIC* `VendorName`
     * - **0x01** *BASIC* `ProductCode`
     * - **0x02** *BASIC* `MajorMinorRevision`
     * - **0x03** *REGULAR* `VendorUrl`
     * - **0x04** *REGULAR* `ProductName`
     * - **0x05** *REGULAR* `ModelName`
     * - **0x06** *REGULAR* `UserApplicationName`
     * - **0x07 - 0x7F** *REGULAR* `Reserved`
     * - **0x80 - 0xFF** *EXTENDED* `Device Specific`
     */
    #[derive(Clone, Debug)]
    pub struct DeviceInfoObject {
        id: u8,
        value: String,
    }
    impl DeviceInfoObject {
        pub fn new(obj_id: u8, value: String) -> Self {
            Self { id: obj_id, value }
        }
        pub fn to_string(&self) -> String {
            self.value.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coil_booleanness() {
        let a: Coil = true.into();
        assert_ne!(a, !a);
        assert_eq!(a, !!a);
        let b: Coil = false.into();
        assert_eq!(a, !b);
    }
}
