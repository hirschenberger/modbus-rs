use std::io;
use std::io::{Write, Read, Cursor};
use std::net::{TcpStream, Shutdown};
use std::time::Duration;
use std::borrow::BorrowMut;
use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};
use bincode::rustc_serialize::{encode, decode};
use bincode::SizeLimit;

use enum_primitive::FromPrimitive;

use {Function, ModbusResult, ModbusExceptionCode, ModbusError, BitValue, Client};

const MODBUS_PROTOCOL_TCP: u16 = 0x0000;
const MODBUS_TCP_DEFAULT_PORT: u16 = 502;
const MODBUS_HEADER_SIZE: usize = 7;
const MODBUS_MAX_READ_COUNT: usize = 0x7d;
const MODBUS_MAX_WRITE_COUNT: usize = 0x79;

/// Context object which holds state for all modbus operations.
pub struct Ctx {
    tid: u16,
    uid: u8,
    stream: TcpStream,
}

impl Ctx {
    /// Create a new context context object and connect it to `addr` on modbus-tcp default
    /// port (502)
    pub fn new(addr: &str) -> io::Result<Ctx> {
        Self::new_with_port(addr, MODBUS_TCP_DEFAULT_PORT)
    }

    /// Create a new context object and connect it to `addr` on port `port`
    pub fn new_with_port(addr: &str, port: u16) -> io::Result<Ctx> {
        match TcpStream::connect((addr, port)) {
            Ok(s) => {
                // set some sane tcp socket options
                let t = Duration::from_secs(5);
                try!(s.set_read_timeout(Some(t)));
                try!(s.set_write_timeout(Some(t)));
                //                try!(s.set_nodelay(true));
                //                try!(s.set_keepalive(None));
                Ok(Ctx {
                    tid: 0,
                    uid: 1,
                    stream: s,
                })
            }
            Err(e) => Err(e),
        }
    }

    // Create a new transaction Id, incrementing the previous one.
    // The Id is wrapping around if the Id reaches `u16::MAX`.
    fn new_tid(&mut self) -> u16 {
        self.tid = self.tid.wrapping_add(1);
        self.tid
    }
}

impl Drop for Ctx {
    fn drop(&mut self) {
        self.stream.shutdown(Shutdown::Both).unwrap();
    }
}

#[derive(RustcEncodable, RustcDecodable)]
#[repr(packed)]
struct Header {
    tid: u16,
    pid: u16,
    len: u16,
    uid: u8,
}

impl Header {
    fn new(ctx: &mut Ctx, len: u16) -> Header {
        Header {
            tid: ctx.new_tid(),
            pid: MODBUS_PROTOCOL_TCP,
            len: len - MODBUS_HEADER_SIZE as u16,
            uid: ctx.uid,
        }
    }
}

/// Read `count` bits starting at address `addr`.
pub fn read_coils(ctx: &mut Ctx, addr: u16, count: u16) -> ModbusResult<Vec<BitValue>> {
    let bytes = try!(read(ctx, Function::ReadCoils(addr, count)));
    let res = unpack_bits(&bytes, count);
    Ok(res)
}

/// Read `count` input bits starting at address `addr`.
pub fn read_discrete_inputs(ctx: &mut Ctx, addr: u16, count: u16) -> ModbusResult<Vec<BitValue>> {
    let bytes = try!(read(ctx, Function::ReadDiscreteInputs(addr, count)));
    let res = unpack_bits(&bytes, count);
    Ok(res)
}

/// Read `count` 16bit registers starting at address `addr`.
pub fn read_holding_registers(ctx: &mut Ctx, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
    let bytes = try!(read(ctx, Function::ReadHoldingRegisters(addr, count)));
    pack_bytes(&bytes[..])
}

/// Read `count` 16bit input registers starting at address `addr`.
pub fn read_input_registers(ctx: &mut Ctx, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
    let bytes = try!(read(ctx, Function::ReadInputRegisters(addr, count)));
    pack_bytes(&bytes[..])
}

fn read(ctx: &mut Ctx, fun: Function) -> ModbusResult<Vec<u8>> {
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

    let header = Header::new(ctx, MODBUS_HEADER_SIZE as u16 + 6u16);
    let mut buff = try!(encode(&header, SizeLimit::Infinite));
    try!(buff.write_u8(fun.code()));
    try!(buff.write_u16::<BigEndian>(addr));
    try!(buff.write_u16::<BigEndian>(count));

    match ctx.stream.write_all(&buff[..]) {
        Ok(_s) => {
            let mut reply = vec![0; MODBUS_HEADER_SIZE + expected_bytes + 2];
            match ctx.stream.read(&mut reply) {
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

/// Write a single coil (bit) to address `addr`.
pub fn write_single_coil(ctx: &mut Ctx, addr: u16, value: BitValue) -> ModbusResult<()> {
    write_single(ctx, Function::WriteSingleCoil(addr, value.code()))
}

/// Write a single 16bit register to address `addr`.
pub fn write_single_register(ctx: &mut Ctx, addr: u16, value: u16) -> ModbusResult<()> {
    write_single(ctx, Function::WriteSingleRegister(addr, value))
}

/// Write a multiple coils (bits) starting at address `addr`.
pub fn write_multiple_coils(ctx: &mut Ctx, addr: u16, values: &[BitValue]) -> ModbusResult<()> {
    let bytes = pack_bits(values);
    write_multiple(ctx,
                   Function::WriteMultipleCoils(addr, values.len() as u16, &bytes[..]))
}

/// Write a multiple 16bit registers starting at address `addr`.
pub fn write_multiple_registers(ctx: &mut Ctx, addr: u16, values: &[u16]) -> ModbusResult<()> {
    let bytes = unpack_bytes(values);
    write_multiple(ctx,
                   Function::WriteMultipleRegisters(addr, values.len() as u16, &bytes[..]))
}

fn write_single(ctx: &mut Ctx, fun: Function) -> ModbusResult<()> {
    let (addr, value) = match fun {
        Function::WriteSingleCoil(a, v) => (a, v),
        Function::WriteSingleRegister(a, v) => (a, v),
        _ => panic!("Unexpected modbus function"),
    };

    let mut buff = vec![0; MODBUS_HEADER_SIZE];  // Header gets filled in later
    try!(buff.write_u8(fun.code()));
    try!(buff.write_u16::<BigEndian>(addr));
    try!(buff.write_u16::<BigEndian>(value));
    write(ctx, &mut buff)
}

fn write_multiple(ctx: &mut Ctx, fun: Function) -> ModbusResult<()> {
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
    write(ctx, &mut buff)
}

fn write(ctx: &mut Ctx, buff: &mut Vec<u8>) -> ModbusResult<()> {
    if buff.len() < 1 || buff.len() > MODBUS_MAX_WRITE_COUNT {
        return Err(ModbusError::InvalidData);
    }
    let header = Header::new(ctx, buff.len() as u16 + 1u16);
    let head_buff = try!(encode(&header, SizeLimit::Infinite));
    {
        let mut start = Cursor::new(buff.borrow_mut());
        try!(start.write(&head_buff[..]));
    }
    match ctx.stream.write_all(&buff[..]) {
        Ok(_s) => {
            let reply = &mut [0; 12];
            match ctx.stream.read(reply) {
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

impl Client for Ctx {

  fn read_discrete_inputs(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<BitValue>> {
    read_discrete_inputs(self, address, quantity)
  }

  fn read_coils(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<BitValue>> {
    read_coils(self, address, quantity)
  }

  fn write_single_coil(&mut self, address: u16, value: BitValue) -> ModbusResult<()> {
    write_single_coil(self, address, value)
  }

  fn write_multiple_coils(&mut self, address: u16, coils: &Vec<BitValue>) -> ModbusResult<()> {
    write_multiple_coils(self, address, coils)
  }

  fn read_input_registers(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<u16>> {
    read_input_registers(self, address, quantity)
  }

  fn read_holding_registers(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<u16>> {
    read_holding_registers(self, address, quantity)
  }

  fn write_single_register(&mut self, address: u16, value: u16) -> ModbusResult<()> {
    write_single_register(self, address, value)
  }

  fn write_multiple_registers(&mut self, address: u16, values: &Vec<u16>) -> ModbusResult<()> {
    write_multiple_registers(self, address, values)
  }

}

#[cfg(test)]
mod tests {
    use super::{pack_bits, unpack_bits, pack_bytes, unpack_bytes};
    use super::super::*;

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
