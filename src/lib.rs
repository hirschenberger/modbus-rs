//! Modbus implementation in pure Rust.
//!
//! # Examples
//!
//! ```
//! # extern crate modbus;
//! # extern crate test_server;
//! # use test_server::start_dummy_server;
//! # fn main() {
//! use modbus::{Client, BitValue};
//! use modbus::tcp;
//! # if cfg!(feature = "modbus-server-tests") {
//! # let (_s, port) = start_dummy_server();
//!
//! // let port = 502;
//! let mut client = tcp::Ctx::new_with_port("127.0.0.1", port).unwrap();
//! assert!(client.write_single_coil(0, BitValue::On).is_ok());
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
use std::io::Cursor;
use byteorder::{BigEndian, ReadBytesExt};
use bincode::rustc_serialize::{DecodingError, EncodingError};

/// The Modbus TCP backend implements a Modbus variant used for communication over TCP/IPv4 networks.
pub mod tcp;

pub trait Client {
  fn read_discrete_inputs(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<BitValue>>;

  fn read_coils(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<BitValue>>;

  fn write_single_coil(&mut self, address: u16, value: BitValue) -> ModbusResult<()>;

  fn write_multiple_coils(&mut self, address: u16, coils: &[BitValue]) -> ModbusResult<()>;

  fn read_input_registers(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<u16>>;

  fn read_holding_registers(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<u16>>;

  fn write_single_register(&mut self, address: u16, value: u16) -> ModbusResult<()>;

  fn write_multiple_registers(&mut self, address: u16, values: &[u16]) -> ModbusResult<()>;
}


impl Client {
  fn unpack_bits(bytes: &[u8], count: u16) -> Vec<BitValue> {
      let mut res = Vec::with_capacity(count as usize);
      for i in 0..count {
          if (bytes[(i / 8u16) as usize] >> (i % 8)) & 0b1 > 0 {
              res.push(BitValue::On);
          } else {
              res.push(BitValue::Off);
          }
      }
      res
  }

  fn pack_bits(bits: &[BitValue]) -> Vec<u8> {
      let bitcount = bits.len();
      let packed_size = bitcount / 8 +
                        if bitcount % 8 > 0 {
          1
      } else {
          0
      };
      let mut res = vec![0; packed_size];
      for (i, b) in bits.iter().enumerate() {
          let v = match *b {
              BitValue::On => 1u8,
              BitValue::Off => 0u8,
          };
          res[(i / 8) as usize] |= v << (i % 8);
      }
      res
  }

  fn unpack_bytes(data: &[u16]) -> Vec<u8> {
      let size = data.len();
      let mut res = Vec::with_capacity(size * 2);
      for b in data {
          res.push((*b >> 8 & 0xff) as u8);
          res.push((*b & 0xff) as u8);
      }
      res
  }

  fn pack_bytes(bytes: &[u8]) -> ModbusResult<Vec<u16>> {
      let size = bytes.len();
      // check if we can create u16s from bytes by packing two u8s together without rest
      if size % 2 != 0 {
          return Err(ModbusError::InvalidData);
      }

      let mut res = Vec::with_capacity(size / 2 + 1);
      let mut rdr = Cursor::new(bytes);
      for _ in 0..size / 2 {
          res.push(try!(rdr.read_u16::<BigEndian>()));
      }
      Ok(res)
  }
}

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
pub enum ModbusExceptionCode {
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
pub enum ModbusError {
    ModbusException(ModbusExceptionCode),
    Io(io::Error),
    InvalidResponse,
    InvalidData
}

impl From<ModbusExceptionCode> for ModbusError {
    fn from(err: ModbusExceptionCode) -> ModbusError {
        ModbusError::ModbusException(err)
    }
}

impl From<io::Error> for ModbusError {
    fn from(err: io::Error) -> ModbusError {
        ModbusError::Io(err)
    }
}

impl From<DecodingError> for ModbusError {
    fn from(_err: DecodingError) -> ModbusError {
        ModbusError::InvalidData
    }
}

impl From<EncodingError> for ModbusError {
    fn from(_err: EncodingError) -> ModbusError {
        ModbusError::InvalidData
    }
}

impl From<byteorder::Error> for ModbusError {
    fn from(_err: byteorder::Error) -> ModbusError {
        ModbusError::InvalidData
    }
}

/// Result type used to nofify success or failure in communication
pub type ModbusResult<T> = std::result::Result<T, ModbusError>;


/// Single bit status values, used in read or write coil functions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BitValue {
    On,
    Off
}

impl BitValue {
    fn code(&self) -> u16 {
        match *self {
            BitValue::On  => 0xff00,
            BitValue::Off => 0x0000
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Client, BitValue};

    #[test]
    fn test_unpack_bits() {

        //assert_eq!(Client::unpack_bits(, 0), &[]);
        assert_eq!(Client::unpack_bits(&[0, 0], 0), &[]);
        assert_eq!(Client::unpack_bits(&[0b1], 1), &[BitValue::On]);
        assert_eq!(Client::unpack_bits(&[0b01], 2), &[BitValue::On, BitValue::Off]);
        assert_eq!(Client::unpack_bits(&[0b10], 2), &[BitValue::Off, BitValue::On]);
        assert_eq!(Client::unpack_bits(&[0b101], 3),
                   &[BitValue::On, BitValue::Off, BitValue::On]);
        assert_eq!(Client::unpack_bits(&[0xff, 0b11], 10), &[BitValue::On; 10]);
    }

    #[test]
    fn test_pack_bits() {
        assert_eq!(Client::pack_bits(&[]), &[]);
        assert_eq!(Client::pack_bits(&[BitValue::On]), &[1]);
        assert_eq!(Client::pack_bits(&[BitValue::Off]), &[0]);
        assert_eq!(Client::pack_bits(&[BitValue::On, BitValue::Off]), &[1]);
        assert_eq!(Client::pack_bits(&[BitValue::Off, BitValue::On]), &[2]);
        assert_eq!(Client::pack_bits(&[BitValue::On, BitValue::On]), &[3]);
        assert_eq!(Client::pack_bits(&[BitValue::On; 8]), &[255]);
        assert_eq!(Client::pack_bits(&[BitValue::On; 9]), &[255, 1]);
        assert_eq!(Client::pack_bits(&[BitValue::Off; 8]), &[0]);
        assert_eq!(Client::pack_bits(&[BitValue::Off; 9]), &[0, 0]);
    }

    #[test]
    fn test_unpack_bytes() {
        assert_eq!(Client::unpack_bytes(&[]), &[]);
        assert_eq!(Client::unpack_bytes(&[0]), &[0, 0]);
        assert_eq!(Client::unpack_bytes(&[1]), &[0, 1]);
        assert_eq!(Client::unpack_bytes(&[0xffff]), &[0xff, 0xff]);
        assert_eq!(Client::unpack_bytes(&[0xffff, 0x0001]), &[0xff, 0xff, 0x00, 0x01]);
        assert_eq!(Client::unpack_bytes(&[0xffff, 0x1001]), &[0xff, 0xff, 0x10, 0x01]);
    }

    #[test]
    fn test_pack_bytes() {
        assert_eq!(Client::pack_bytes(&[]).unwrap(), &[]);
        assert_eq!(Client::pack_bytes(&[0, 0]).unwrap(), &[0]);
        assert_eq!(Client::pack_bytes(&[0, 1]).unwrap(), &[1]);
        assert_eq!(Client::pack_bytes(&[1, 0]).unwrap(), &[256]);
        assert_eq!(Client::pack_bytes(&[1, 1]).unwrap(), &[257]);
        assert_eq!(Client::pack_bytes(&[0, 1, 0, 2]).unwrap(), &[1, 2]);
        assert_eq!(Client::pack_bytes(&[1, 1, 1, 2]).unwrap(), &[257, 258]);
        assert!(Client::pack_bytes(&[1]).is_err());
        assert!(Client::pack_bytes(&[1, 2, 3]).is_err());
    }
}
