use std::u16;
use std::net::{TcpStream, Shutdown};
use std::io;
use std::io::{Write, Read, Cursor};
use std::time::Duration;
use std::borrow::BorrowMut;
use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};
use bincode::rustc_serialize::{encode, decode};
use bincode::SizeLimit;

use enum_primitive::FromPrimitive;

use {Function, ModbusResult, ModbusExceptionCode, IoError, BitValue};

const MODBUS_PROTOCOL_TCP: u16 = 0x0000;
const MODBUS_TCP_DEFAULT_PORT: u16 = 502;
const MODBUS_HEADER_SIZE: usize = 7;
const MODBUS_MAX_READ_COUNT: usize = 0x7d;
const MODBUS_MAX_WRITE_COUNT: usize = 0x79;

/// Context object which holds state for all modbus operations.
pub struct Ctx {
    tid: u16,
    uid: u8,
    stream: TcpStream
}

impl Ctx {
    /// Create a new context `Ctx` context object and connect it to `addr` on modbus-tcp default
    /// port (502)
    pub fn new(addr: &str) -> io::Result<Ctx> {
        Self::new_with_port(addr, MODBUS_TCP_DEFAULT_PORT)
    }

    /// Create a new context `Ctx` context object and connect it to `addr` on port `port`
    pub fn new_with_port(addr: &str, port: u16) -> io::Result<Ctx> {
        match TcpStream::connect((addr, port)) {
            Ok(s) => {
                // set some sane tcp socket options
                let t = Duration::from_secs(5);
                try!(s.set_read_timeout(Some(t)));
                try!(s.set_write_timeout(Some(t)));
//                try!(s.set_nodelay(true));
//                try!(s.set_keepalive(None));
                Ok(Ctx { tid: 0, uid: 1, stream: s })
            }
            Err(e) => Err(e)
        }
    }

    /// Create a new transaction Id, incrementing the previous one.
    ///
    /// The Id is wrapping around if the Id reaches `u16::MAX`.
    fn new_tid(&mut self) -> u16 {
        // wrap around or increment
        if self.tid  < u16::MAX {
            self.tid += 1u16;
        } else {
            self.tid = 0u16;
        }
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
    uid: u8
}

impl Header {
    fn new(ctx: &mut Ctx, len: u16) -> Header {
        Header {
            tid: ctx.new_tid(),
            pid: MODBUS_PROTOCOL_TCP,
            len: len,
            uid: ctx.uid
        }
    }
}


pub fn write_single_coil(ctx: &mut Ctx, addr: u16, value: BitValue) -> ModbusResult<()>
{
    write_single(ctx, Function::WriteSingleCoil(addr, value as u16))
}

pub fn write_single_register(ctx: &mut Ctx, addr: u16, value: u16) -> ModbusResult<()>
{
    write_single(ctx, Function::WriteSingleRegister(addr, value))
}

fn write_single(ctx: &mut Ctx, fun: Function) -> ModbusResult<()>
{
    let (addr, value) = match fun {
        Function::WriteSingleCoil(a, v)     => (a, v),
        Function::WriteSingleRegister(a, v) => (a, v),
        _ => panic!("Unexpected modbus function")
    };

    let mut buff = vec![0; MODBUS_HEADER_SIZE];  // Header gets filled in later
    buff.write_u8(fun.code()).unwrap();
    buff.write_u16::<BigEndian>(addr).unwrap();
    buff.write_u16::<BigEndian>(value).unwrap();
    write(ctx, &mut buff)
}

fn write(ctx: &mut Ctx, buff: &mut Vec<u8>) -> ModbusResult<()> {
    if buff.len() > MODBUS_MAX_WRITE_COUNT {
        return Err(IoError::ModbusExceptionCode(ModbusExceptionCode::IllegalDataValue));
    }
    let header = Header::new(ctx, (buff.len() - MODBUS_HEADER_SIZE + 1) as u16);
    let head_buff = encode(&header, SizeLimit::Infinite).unwrap();
    {
        let mut start = Cursor::new(buff.borrow_mut());
        start.write(&head_buff[..]).unwrap();
    }
    match ctx.stream.write_all(&buff[..]) {
        Ok(_s) => {
                let reply = &mut [0; 12];
                match ctx.stream.read(reply) {
                    Ok(_s) => {
                        let resp_hd = decode(&reply[..MODBUS_HEADER_SIZE]).unwrap();
                        validate_response_header(&header, &resp_hd).and(
                            validate_response_code(&buff, reply))
                    }
                    Err(_e) => Err(IoError::Communication)
                }
        }
        Err(_e) => Err(IoError::Communication)
    }
}

fn validate_response_header(req: &Header, resp: &Header) -> ModbusResult<()> {
    if req.tid != resp.tid || resp.pid != MODBUS_PROTOCOL_TCP {
        Err(IoError::Communication)
    }
    else {
        Ok(())
    }
}

fn validate_response_code(req: &[u8], resp: &[u8]) -> ModbusResult<()> {
    if req[7] + 0x80  == resp[7] {
        let code = ModbusExceptionCode::from_u8(resp[8]).unwrap();
        Err(IoError::ModbusExceptionCode(code))
    }
    else if req[7] != resp[7] {
        Err(IoError::Communication)
    }
    else {
        Ok(())
    }
}

#[cfg(test)]
fn start_dummy_server(port: &str) -> ChildKiller {
    use std::process::{Command, Stdio};
    use std::thread::sleep_ms;
    let ck = ChildKiller(Command::new("./test/diagslave")
                        .arg("-m").arg("tcp")
                        .arg("-p").arg(port)
                        .stdout(Stdio::null())
                        .spawn()
                        .unwrap_or_else(|e| { panic!("failed to execute process: {}", e) }));
    sleep_ms(500);
    ck
}

#[cfg(test)]
use std::process::Child;
#[cfg(test)]
struct ChildKiller(Child);

#[cfg(test)]
impl Drop for ChildKiller {
    fn drop(&mut self) {
        self.0.kill().unwrap();
    }
}

#[test]
fn test_packet_tid_creation() {
    let _s = start_dummy_server("2222");
    let mut ctx = Ctx::new_with_port("127.0.0.1", 2222).unwrap();
    let mut hd = Header::new(&mut ctx, 10);
    assert!(hd.tid == 1u16);
    hd = Header::new(&mut ctx, 10);
    assert!(hd.tid == 2u16);
    ctx.tid = u16::MAX;
    hd = Header::new(&mut ctx, 10);
    assert!(hd.tid == 0);
}

#[test]
fn test_write_single_coil() {
    let _s = start_dummy_server("2223");
    let mut ctx = Ctx::new_with_port("127.0.0.1", 2223).unwrap();
    for i in 0..10 {
        assert!(write_single_coil(&mut ctx, i, BitValue::On).is_ok());
    }
}

#[test]
fn test_write_single_register() {
    let _s = start_dummy_server("2224");
    let mut ctx = Ctx::new_with_port("127.0.0.1", 2224).unwrap();
    for i in 0..10 {
        assert!(write_single_register(&mut ctx, i, i).is_ok());
    }
}
