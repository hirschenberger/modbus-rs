use std::io;
use std::io::{Write, Read, Cursor};
use std::net::{TcpStream, Shutdown};
use std::time::Duration;
use std::borrow::BorrowMut;
use byteorder::{BigEndian, WriteBytesExt};
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

    fn read(self: &mut Self, fun: Function) -> ModbusResult<Vec<u8>> {
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
                        try!(Ctx::validate_response_header(&header, &resp_hd));
                        try!(Ctx::validate_response_code(&buff, &reply[..]));
                        Ctx::get_reply_data(&reply, expected_bytes)
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

    fn write_single(self: &mut Self, fun: Function) -> ModbusResult<()> {
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

    fn write_multiple(self: &mut Self, fun: Function) -> ModbusResult<()> {
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

    fn write(self: &mut Self, buff: &mut [u8]) -> ModbusResult<()> {
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
                        try!(Ctx::validate_response_header(&header, &resp_hd));
                        Ctx::validate_response_code(&buff, reply)
                    }
                    Err(e) => Err(ModbusError::Io(e)),
                }
            }
            Err(e) => Err(ModbusError::Io(e)),
        }
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

impl Client for Ctx {
    /// Read `count` bits starting at address `addr`.
    fn read_coils(self: &mut Self, addr: u16, count: u16) -> ModbusResult<Vec<BitValue>> {
        let bytes = try!(self.read(Function::ReadCoils(addr, count)));
        let res = Client::unpack_bits(&bytes, count);
        Ok(res)
    }

    /// Read `count` input bits starting at address `addr`.
    fn read_discrete_inputs(self: &mut Self, addr: u16, count: u16) -> ModbusResult<Vec<BitValue>> {
        let bytes = try!(self.read(Function::ReadDiscreteInputs(addr, count)));
        let res = Client::unpack_bits(&bytes, count);
        Ok(res)
    }

    /// Read `count` 16bit registers starting at address `addr`.
    fn read_holding_registers(self: &mut Self, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
        let bytes = try!(self.read(Function::ReadHoldingRegisters(addr, count)));
        Client::pack_bytes(&bytes[..])
    }

    /// Read `count` 16bit input registers starting at address `addr`.
    fn read_input_registers(self: &mut Self, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
        let bytes = try!(self.read(Function::ReadInputRegisters(addr, count)));
        Client::pack_bytes(&bytes[..])
    }


    /// Write a single coil (bit) to address `addr`.
    fn write_single_coil(self: &mut Self, addr: u16, value: BitValue) -> ModbusResult<()> {
        self.write_single(Function::WriteSingleCoil(addr, value.code()))
    }

    /// Write a single 16bit register to address `addr`.
    fn write_single_register(self: &mut Self, addr: u16, value: u16) -> ModbusResult<()> {
        self.write_single(Function::WriteSingleRegister(addr, value))
    }

    /// Write a multiple coils (bits) starting at address `addr`.
    fn write_multiple_coils(self: &mut Self, addr: u16, values: &[BitValue]) -> ModbusResult<()> {
        let bytes = Client::pack_bits(values);
        self.write_multiple(Function::WriteMultipleCoils(addr, values.len() as u16, &bytes[..]))
    }

    /// Write a multiple 16bit registers starting at address `addr`.
    fn write_multiple_registers(self: &mut Self, addr: u16, values: &[u16]) -> ModbusResult<()> {
        let bytes = Client::unpack_bytes(values);
        self.write_multiple(Function::WriteMultipleRegisters(addr, values.len() as u16, &bytes[..]))
    }
}
