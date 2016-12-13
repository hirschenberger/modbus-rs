use std::io::{self, Write, Read, Cursor};
use std::net::{TcpStream, Shutdown};
use std::time::Duration;
use std::borrow::BorrowMut;
use byteorder::{BigEndian, WriteBytesExt};
use bincode::rustc_serialize::{encode, decode};
use bincode::SizeLimit;
use enum_primitive::FromPrimitive;
use {Function, Reason, Result, ExceptionCode, Error, Coil, binary, Client};

const MODBUS_PROTOCOL_TCP: u16 = 0x0000;
const MODBUS_TCP_DEFAULT_PORT: u16 = 502;
const MODBUS_HEADER_SIZE: usize = 7;
const MODBUS_MAX_PACKET_SIZE: usize = 260;

/// Config structure for more control over the tcp socket settings
#[derive(Clone, Copy)]
pub struct Config {
    /// The TCP port to use for communication (Default: `502`)
    pub tcp_port: u16,
    /// Timeout when reading from the TCP socket (Default: `infinite`)
    pub tcp_read_timeout: Option<Duration>,
    /// Timeout when writing to the TCP socket (Default: `infinite`)
    pub tcp_write_timeout: Option<Duration>,
    /// The modbus Unit Identifier used in the modbus layer (Default: `1`)
    pub modbus_uid: u8,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            tcp_port: MODBUS_TCP_DEFAULT_PORT,
            tcp_read_timeout: None,
            tcp_write_timeout: None,
            modbus_uid: 1,
        }
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
    fn new(transport: &mut Transport, len: u16) -> Header {
        Header {
            tid: transport.new_tid(),
            pid: MODBUS_PROTOCOL_TCP,
            len: len - MODBUS_HEADER_SIZE as u16,
            uid: transport.uid,
        }
    }
}

/// Context object which holds state for all modbus operations.
pub struct Transport {
    tid: u16,
    uid: u8,
    stream: TcpStream,
}

impl Transport {
    /// Create a new context context object and connect it to `addr` on modbus-tcp default
    /// port (502)
    pub fn new(addr: &str) -> io::Result<Transport> {
        Self::new_with_cfg(addr, Config::default())
    }

    /// Create a new context object and connect it to `addr` on port `port`
    pub fn new_with_cfg(addr: &str, cfg: Config) -> io::Result<Transport> {
        match TcpStream::connect((addr, cfg.tcp_port)) {
            Ok(s) => {
                s.set_read_timeout(cfg.tcp_read_timeout)?;
                s.set_write_timeout(cfg.tcp_write_timeout)?;
                s.set_nodelay(true)?;
                Ok(Transport {
                    tid: 0,
                    uid: cfg.modbus_uid,
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

    fn read(self: &mut Self, fun: Function) -> Result<Vec<u8>> {
        let packed_size = |v: u16| {
            v / 8 +
            if v % 8 > 0 {
                1
            } else {
                0
            }
        };
        let (addr, count, expected_bytes) = match fun {
            Function::ReadCoils(a, c) |
            Function::ReadDiscreteInputs(a, c) => (a, c, packed_size(c) as usize),
            Function::ReadHoldingRegisters(a, c) |
            Function::ReadInputRegisters(a, c) => (a, c, 2 * c as usize),
            _ => return Err(Error::InvalidFunction),
        };

        if count < 1 {
            return Err(Error::InvalidData(Reason::RecvBufferEmpty));
        }

        if count as usize > MODBUS_MAX_PACKET_SIZE {
            return Err(Error::InvalidData(Reason::UnexpectedReplySize));
        }

        let header = Header::new(self, MODBUS_HEADER_SIZE as u16 + 6u16);
        let mut buff = encode(&header, SizeLimit::Infinite)?;
        buff.write_u8(fun.code())?;
        buff.write_u16::<BigEndian>(addr)?;
        buff.write_u16::<BigEndian>(count)?;

        match self.stream.write_all(&buff) {
            Ok(_s) => {
                let mut reply = vec![0; MODBUS_HEADER_SIZE + expected_bytes + 2];
                match self.stream.read(&mut reply) {
                    Ok(_s) => {
                        let resp_hd = decode(&reply[..MODBUS_HEADER_SIZE])?;
                        Transport::validate_response_header(&header, &resp_hd)?;
                        Transport::validate_response_code(&buff, &reply)?;
                        Transport::get_reply_data(&reply, expected_bytes)
                    }
                    Err(e) => Err(Error::Io(e)),
                }
            }
            Err(e) => Err(Error::Io(e)),
        }
    }

    fn validate_response_header(req: &Header, resp: &Header) -> Result<()> {
        if req.tid != resp.tid || resp.pid != MODBUS_PROTOCOL_TCP {
            Err(Error::InvalidResponse)
        } else {
            Ok(())
        }
    }

    fn validate_response_code(req: &[u8], resp: &[u8]) -> Result<()> {
        if req[7] + 0x80 == resp[7] {
            match ExceptionCode::from_u8(resp[8]) {
                Some(code) => Err(Error::Exception(code)),
                None => Err(Error::InvalidResponse),
            }
        } else if req[7] == resp[7] {
            Ok(())
        } else {
            Err(Error::InvalidResponse)
        }
    }

    fn get_reply_data(reply: &[u8], expected_bytes: usize) -> Result<Vec<u8>> {
        if reply[8] as usize != expected_bytes ||
           reply.len() != MODBUS_HEADER_SIZE + expected_bytes + 2 {
            Err(Error::InvalidData(Reason::UnexpectedReplySize))
        } else {
            let mut d = Vec::new();
            d.extend_from_slice(&reply[MODBUS_HEADER_SIZE + 2..]);
            Ok(d)
        }
    }

    fn write_single(self: &mut Self, fun: Function) -> Result<()> {
        let (addr, value) = match fun {
            Function::WriteSingleCoil(a, v) |
            Function::WriteSingleRegister(a, v) => (a, v),
            _ => return Err(Error::InvalidFunction),
        };

        let mut buff = vec![0; MODBUS_HEADER_SIZE];  // Header gets filled in later
        buff.write_u8(fun.code())?;
        buff.write_u16::<BigEndian>(addr)?;
        buff.write_u16::<BigEndian>(value)?;
        self.write(&mut buff)
    }

    fn write_multiple(self: &mut Self, fun: Function) -> Result<()> {
        let (addr, quantity, values) = match fun {
            Function::WriteMultipleCoils(a, q, v) |
            Function::WriteMultipleRegisters(a, q, v) => (a, q, v),
            _ => return Err(Error::InvalidFunction),
        };

        let mut buff = vec![0; MODBUS_HEADER_SIZE];  // Header gets filled in later
        buff.write_u8(fun.code())?;
        buff.write_u16::<BigEndian>(addr)?;
        buff.write_u16::<BigEndian>(quantity)?;
        buff.write_u8(values.len() as u8)?;
        for v in values {
            buff.write_u8(*v)?;
        }
        self.write(&mut buff)
    }

    fn write(self: &mut Self, buff: &mut [u8]) -> Result<()> {
        if buff.len() < 1 {
            return Err(Error::InvalidData(Reason::SendBufferEmpty));
        }

        if buff.len() > MODBUS_MAX_PACKET_SIZE {
            return Err(Error::InvalidData(Reason::SendBufferTooBig));
        }

        let header = Header::new(self, buff.len() as u16 + 1u16);
        let head_buff = encode(&header, SizeLimit::Infinite)?;
        {
            let mut start = Cursor::new(buff.borrow_mut());
            start.write(&head_buff)?;
        }
        match self.stream.write_all(buff) {
            Ok(_s) => {
                let reply = &mut [0; 12];
                match self.stream.read(reply) {
                    Ok(_s) => {
                        let resp_hd = decode(reply)?;
                        Transport::validate_response_header(&header, &resp_hd)?;
                        Transport::validate_response_code(buff, reply)
                    }
                    Err(e) => Err(Error::Io(e)),
                }
            }
            Err(e) => Err(Error::Io(e)),
        }
    }

    pub fn close(self: &mut Self) -> Result<()> {
        self.stream.shutdown(Shutdown::Both).map_err(Error::Io)
    }
}

impl Client for Transport {
    /// Read `count` bits starting at address `addr`.
    fn read_coils(self: &mut Self, addr: u16, count: u16) -> Result<Vec<Coil>> {
        let bytes = self.read(Function::ReadCoils(addr, count))?;
        Ok(binary::unpack_bits(&bytes, count))
    }

    /// Read `count` input bits starting at address `addr`.
    fn read_discrete_inputs(self: &mut Self, addr: u16, count: u16) -> Result<Vec<Coil>> {
        let bytes = self.read(Function::ReadDiscreteInputs(addr, count))?;
        Ok(binary::unpack_bits(&bytes, count))
    }

    /// Read `count` 16bit registers starting at address `addr`.
    fn read_holding_registers(self: &mut Self, addr: u16, count: u16) -> Result<Vec<u16>> {
        let bytes = self.read(Function::ReadHoldingRegisters(addr, count))?;
        binary::pack_bytes(&bytes[..])
    }

    /// Read `count` 16bit input registers starting at address `addr`.
    fn read_input_registers(self: &mut Self, addr: u16, count: u16) -> Result<Vec<u16>> {
        let bytes = self.read(Function::ReadInputRegisters(addr, count))?;
        binary::pack_bytes(&bytes[..])
    }

    /// Write a single coil (bit) to address `addr`.
    fn write_single_coil(self: &mut Self, addr: u16, value: Coil) -> Result<()> {
        self.write_single(Function::WriteSingleCoil(addr, value.code()))
    }

    /// Write a single 16bit register to address `addr`.
    fn write_single_register(self: &mut Self, addr: u16, value: u16) -> Result<()> {
        self.write_single(Function::WriteSingleRegister(addr, value))
    }

    /// Write a multiple coils (bits) starting at address `addr`.
    fn write_multiple_coils(self: &mut Self, addr: u16, values: &[Coil]) -> Result<()> {
        let bytes = binary::pack_bits(values);
        self.write_multiple(Function::WriteMultipleCoils(addr, values.len() as u16, &bytes))
    }

    /// Write a multiple 16bit registers starting at address `addr`.
    fn write_multiple_registers(self: &mut Self, addr: u16, values: &[u16]) -> Result<()> {
        let bytes = binary::unpack_bytes(values);
        self.write_multiple(Function::WriteMultipleRegisters(addr, values.len() as u16, &bytes))
    }
}
