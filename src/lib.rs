//! Modbus implementation in pure Rust.
//!
//! # Examples
//!
//! ```
//! # extern crate modbus;
//! # extern crate test_server;
//! # use test_server::start_dummy_server;
//! # fn main() {
//! use modbus::*;
//! # if cfg!(feature = "modbus-server-tests") {
//! # let (_s, port) = start_dummy_server();
//!
//! // let port = 502;
//! let mut ctx = Context::new_with_port("127.0.0.1", port).unwrap();
//! assert!(ctx.write_single_coil(0, BitValue::On).is_ok());
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

use std::borrow::BorrowMut;
use std::io;
use std::io::{Write, Read, Cursor};
use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};
use bincode::rustc_serialize::{DecodingError, EncodingError};
use bincode::rustc_serialize::{encode, decode};
use bincode::SizeLimit;
use enum_primitive::FromPrimitive;



/// The Modbus TCP backend implements a Modbus variant used for communication over TCP/IPv4 networks.
pub mod tcp;

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

const MODBUS_PROTOCOL_TCP: u16 = 0x0000;
const MODBUS_HEADER_SIZE: usize = 7;
const MODBUS_MAX_READ_COUNT: usize = 0x7d;
const MODBUS_MAX_WRITE_COUNT: usize = 0x79;

#[derive(RustcEncodable, RustcDecodable)]
#[repr(packed)]
struct Header {
    tid: u16,
    pid: u16,
    len: u16,
    uid: u8,
}

impl Header {
    fn new<Stream>(ctx: &mut Context<Stream>, len: u16) -> Header {
        Header {
            tid: ctx.new_tid(),
            pid: MODBUS_PROTOCOL_TCP,
            len: len - MODBUS_HEADER_SIZE as u16,
            uid: ctx.uid,
        }
    }
}

pub struct Context<Stream> {
    tid: u16,
    uid: u8,
    stream: Stream,
}

impl<Stream> Context<Stream> {
    // Create a new transaction Id, incrementing the previous one.
    // The Id is wrapping around if the Id reaches `u16::MAX`.
    fn new_tid(&mut self) -> u16 {
        self.tid = self.tid.wrapping_add(1);
        self.tid
    }
}

impl<Stream: Write + Read> Context<Stream> {
    pub fn new(stream: Stream) -> Self {
        Context { tid: 0, uid: 1, stream: stream }
    }

    /// Read `count` bits starting at address `addr`.
    pub fn read_coils(&mut self, addr: u16, count: u16) -> ModbusResult<Vec<BitValue>> {
        let bytes = try!(self.read(Function::ReadCoils(addr, count)));
        let res = unpack_bits(&bytes, count);
        Ok(res)
    }

    /// Read `count` input bits starting at address `addr`.
    pub fn read_discrete_inputs(&mut self, addr: u16, count: u16) -> ModbusResult<Vec<BitValue>> {
        let bytes = try!(self.read(Function::ReadDiscreteInputs(addr, count)));
        let res = unpack_bits(&bytes, count);
        Ok(res)
    }

    /// Read `count` 16bit registers starting at address `addr`.
    pub fn read_holding_registers(&mut self, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
        let bytes = try!(self.read(Function::ReadHoldingRegisters(addr, count)));
        pack_bytes(&bytes[..])
    }

    /// Read `count` 16bit input registers starting at address `addr`.
    pub fn read_input_registers(&mut self, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
        let bytes = try!(self.read(Function::ReadInputRegisters(addr, count)));
        pack_bytes(&bytes[..])
    }

    /// Write a single coil (bit) to address `addr`.
    pub fn write_single_coil(&mut self, addr: u16, value: BitValue) -> ModbusResult<()> {
        self.write_single(Function::WriteSingleCoil(addr, value.code()))
    }

    /// Write a single 16bit register to address `addr`.
    pub fn write_single_register(&mut self, addr: u16, value: u16) -> ModbusResult<()> {
        self.write_single(Function::WriteSingleRegister(addr, value))
    }

    /// Write a multiple coils (bits) starting at address `addr`.
    pub fn write_multiple_coils(&mut self, addr: u16, values: &[BitValue]) -> ModbusResult<()> {
        let bytes = pack_bits(values);
        self.write_multiple(Function::WriteMultipleCoils(addr, values.len() as u16, &bytes[..]))
    }

    /// Write a multiple 16bit registers starting at address `addr`.
    pub fn write_multiple_registers(&mut self, addr: u16, values: &[u16]) -> ModbusResult<()> {
        let bytes = unpack_bytes(values);
        self.write_multiple(Function::WriteMultipleRegisters(addr, values.len() as u16, &bytes[..]))
    }

    fn read(&mut self, fun: Function) -> ModbusResult<Vec<u8>> {
        let packed_size = |v: u16| {
            v / 8 +
            if v % 8 > 0 {
                1
            } else {
                0
            }
        };
        let (addr, count, expected_bytes) = match fun {
            Function::ReadCoils(a, c) => (a, c, packed_size(c) as usize),
            Function::ReadDiscreteInputs(a, c) => (a, c, packed_size(c) as usize),
            Function::ReadHoldingRegisters(a, c) => (a, c, 2 * c as usize),
            Function::ReadInputRegisters(a, c) => (a, c, 2 * c as usize),
            _ => panic!("Unexpected modbus function"),
        };

        if count < 1 || count as usize > MODBUS_MAX_READ_COUNT {
            return Err(ModbusError::InvalidData);
        }

        let header = Header::new(self, MODBUS_HEADER_SIZE as u16 + 6u16);
        let mut buff = try!(encode(&header, SizeLimit::Infinite));
        try!(buff.write_u8(fun.code()));
        try!(buff.write_u16::<BigEndian>(addr));
        try!(buff.write_u16::<BigEndian>(count));

        match self.stream.write_all(&buff[..]) {
            Ok(_s) => {
                let mut reply = vec![0; MODBUS_HEADER_SIZE + expected_bytes + 2];
                match self.stream.read(&mut reply) {
                    Ok(_s) => {
                        let resp_hd = try!(decode(&reply[..MODBUS_HEADER_SIZE]));
                        try!(validate_response_header(&header, &resp_hd));
                        try!(validate_response_code(&buff, &reply[..]));
                        get_reply_data(&reply, expected_bytes)
                    }
                    Err(e) => Err(ModbusError::Io(e)),
                }
            }
            Err(e) => Err(ModbusError::Io(e)),
        }
    }

    fn write(&mut self, buff: &mut Vec<u8>) -> ModbusResult<()> {
        if buff.len() < 1 || buff.len() > MODBUS_MAX_WRITE_COUNT {
            return Err(ModbusError::InvalidData);
        }
        let header = Header::new(self, buff.len() as u16 + 1u16);
        let head_buff = try!(encode(&header, SizeLimit::Infinite));
        {
            let mut start = Cursor::new(buff.borrow_mut());
            try!(start.write(&head_buff[..]));
        }
        match self.stream.write_all(&buff[..]) {
            Ok(_s) => {
                let reply = &mut [0; 12];
                match self.stream.read(reply) {
                    Ok(_s) => {
                        let resp_hd = try!(decode(&reply[..MODBUS_HEADER_SIZE]));
                        try!(validate_response_header(&header, &resp_hd));
                        validate_response_code(&buff, reply)
                    }
                    Err(e) => Err(ModbusError::Io(e)),
                }
            }
            Err(e) => Err(ModbusError::Io(e)),
        }
    }

    fn write_single(&mut self, fun: Function) -> ModbusResult<()> {
        let (addr, value) = match fun {
            Function::WriteSingleCoil(a, v) => (a, v),
            Function::WriteSingleRegister(a, v) => (a, v),
            _ => panic!("Unexpected modbus function"),
        };

        let mut buff = vec![0; MODBUS_HEADER_SIZE];  // Header gets filled in later
        try!(buff.write_u8(fun.code()));
        try!(buff.write_u16::<BigEndian>(addr));
        try!(buff.write_u16::<BigEndian>(value));
        self.write(&mut buff)
    }

    fn write_multiple(&mut self, fun: Function) -> ModbusResult<()> {
        let (addr, quantity, values) = match fun {
            Function::WriteMultipleCoils(a, q, v) => (a, q, v),
            Function::WriteMultipleRegisters(a, q, v) => (a, q, v),
            _ => panic!("Unexpected modbus function"),
        };

        let mut buff = vec![0; MODBUS_HEADER_SIZE];  // Header gets filled in later
        try!(buff.write_u8(fun.code()));
        try!(buff.write_u16::<BigEndian>(addr));
        try!(buff.write_u16::<BigEndian>(quantity));
        try!(buff.write_u8(values.len() as u8));
        for v in values {
            try!(buff.write_u8(*v));
        }
        self.write(&mut buff)
    }
}

fn get_reply_data(reply: &[u8], expected_bytes: usize) -> ModbusResult<Vec<u8>> {
    if reply[8] as usize != expected_bytes ||
       reply.len() != MODBUS_HEADER_SIZE + expected_bytes + 2 {
        Err(ModbusError::InvalidData)
    } else {

        let mut d = Vec::new();
        d.extend(reply[MODBUS_HEADER_SIZE + 2..].iter());
        Ok(d)
    }
}

fn validate_response_header(req: &Header, resp: &Header) -> ModbusResult<()> {
    if req.tid != resp.tid || resp.pid != MODBUS_PROTOCOL_TCP {
        Err(ModbusError::InvalidResponse)
    } else {
        Ok(())
    }
}


fn validate_response_code(req: &[u8], resp: &[u8]) -> ModbusResult<()> {
    if req[7] + 0x80 == resp[7] {
        let code = ModbusExceptionCode::from_u8(resp[8]).unwrap();
        Err(ModbusError::ModbusException(code))
    } else if req[7] != resp[7] {
        Err(ModbusError::InvalidResponse)
    } else {
        Ok(())
    }
}

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

#[cfg(test)]
mod tests {
    use super::{pack_bits, unpack_bits, pack_bytes, unpack_bytes};
    use super::*;

    #[test]
    fn test_unpack_bits() {
        assert_eq!(unpack_bits(&[], 0), &[]);
        assert_eq!(unpack_bits(&[0, 0], 0), &[]);
        assert_eq!(unpack_bits(&[0b1], 1), &[BitValue::On]);
        assert_eq!(unpack_bits(&[0b01], 2), &[BitValue::On, BitValue::Off]);
        assert_eq!(unpack_bits(&[0b10], 2), &[BitValue::Off, BitValue::On]);
        assert_eq!(unpack_bits(&[0b101], 3),
                   &[BitValue::On, BitValue::Off, BitValue::On]);
        assert_eq!(unpack_bits(&[0xff, 0b11], 10), &[BitValue::On; 10]);
    }

    #[test]
    fn test_pack_bits() {
        assert_eq!(pack_bits(&[]), &[]);
        assert_eq!(pack_bits(&[BitValue::On]), &[1]);
        assert_eq!(pack_bits(&[BitValue::Off]), &[0]);
        assert_eq!(pack_bits(&[BitValue::On, BitValue::Off]), &[1]);
        assert_eq!(pack_bits(&[BitValue::Off, BitValue::On]), &[2]);
        assert_eq!(pack_bits(&[BitValue::On, BitValue::On]), &[3]);
        assert_eq!(pack_bits(&[BitValue::On; 8]), &[255]);
        assert_eq!(pack_bits(&[BitValue::On; 9]), &[255, 1]);
        assert_eq!(pack_bits(&[BitValue::Off; 8]), &[0]);
        assert_eq!(pack_bits(&[BitValue::Off; 9]), &[0, 0]);
    }

    #[test]
    fn test_unpack_bytes() {
        assert_eq!(unpack_bytes(&[]), &[]);
        assert_eq!(unpack_bytes(&[0]), &[0, 0]);
        assert_eq!(unpack_bytes(&[1]), &[0, 1]);
        assert_eq!(unpack_bytes(&[0xffff]), &[0xff, 0xff]);
        assert_eq!(unpack_bytes(&[0xffff, 0x0001]), &[0xff, 0xff, 0x00, 0x01]);
        assert_eq!(unpack_bytes(&[0xffff, 0x1001]), &[0xff, 0xff, 0x10, 0x01]);
    }

    #[test]
    fn test_pack_bytes() {
        assert_eq!(pack_bytes(&[]).unwrap(), &[]);
        assert_eq!(pack_bytes(&[0, 0]).unwrap(), &[0]);
        assert_eq!(pack_bytes(&[0, 1]).unwrap(), &[1]);
        assert_eq!(pack_bytes(&[1, 0]).unwrap(), &[256]);
        assert_eq!(pack_bytes(&[1, 1]).unwrap(), &[257]);
        assert_eq!(pack_bytes(&[0, 1, 0, 2]).unwrap(), &[1, 2]);
        assert_eq!(pack_bytes(&[1, 1, 1, 2]).unwrap(), &[257, 258]);
        assert!(pack_bytes(&[1]).is_err());
        assert!(pack_bytes(&[1, 2, 3]).is_err());
    }
}
