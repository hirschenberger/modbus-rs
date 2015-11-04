use std::u16;
use std::net::{TcpStream};
use std::io::{Write, Read, Result};
use std::time::Duration;
use std::process::{Command, Child, Stdio};
use std::thread;

use byteorder::{BigEndian, WriteBytesExt};
use bincode::rustc_serialize::{encode, decode};
use bincode::SizeLimit;

use {FunctionCode, ModbusResult, ExceptionCode, BitValue};

const PROTOCOL_MODBUS_TCP: u16 = 0x0000;
const MODBUS_TCP_DEFAULT_PORT: u16 = 502;

/// Context object which holds state for all modbus operations.
pub struct Ctx {
    tid: u16,
    uid: u8,
    stream: TcpStream
}

impl Ctx {
    /// Create a new context `Ctx` context object and connect it to `addr` on modbus-tcp default
    /// port (502)
    pub fn new(addr: &str) -> Result<Ctx> {
        Self::new_with_port(addr, MODBUS_TCP_DEFAULT_PORT)
    }

    /// Create a new context `Ctx` context object and connect it to `addr` on port `port`
    pub fn new_with_port(addr: &str, port: u16) -> Result<Ctx> {
        match TcpStream::connect((addr, port)) {
            Ok(s) => {
                // set some sane tcp socket options
                let t = Duration::from_secs(5);
                try!(s.set_read_timeout(Some(t)));
                try!(s.set_write_timeout(Some(t)));
//                try!(s.set_nodelay(true));
//                try!(s.set_keepalive(None));
                Ok(Ctx { tid: 0, uid: 0, stream: s })
            }
            Err(e) => Err(e)
        }
    }

    /// Create a new transaction Id, incrementing the previous one.
    ///
    /// The Id is wrapping around if the Id reaches `u16::MAX`.
    fn new_tid(self: &mut Self) -> u16 {
        // wrap around or increment
        if self.tid  < u16::MAX {
            self.tid += 1u16;
        } else {
            self.tid = 0u16;
        }
        self.tid
    }
}

#[derive(RustcEncodable, RustcDecodable)]
#[repr(packed)]
struct Packet {
    tid: u16,
    pid: u16,
    len: u16,
    uid: u8,
    fun: u8,
    addr: u16,
}

impl Packet {
    fn new(ctx: &mut Ctx, fun: FunctionCode, addr: u16) -> Packet {
        Packet {
            tid: ctx.new_tid(),
            pid: PROTOCOL_MODBUS_TCP,
            len: 4u16,
            uid: ctx.uid,
            fun: fun as u8,
            addr: addr,
        }
    }

    fn encode_with_data(self: &mut Self, data: &mut Vec<u8>) -> Vec<u8>
    {
        self.len += data.len() as u16;
        let mut pack: Vec<u8> = encode(self, SizeLimit::Infinite).unwrap();
        pack.append(data);
        pack
    }
}

fn write_single(ctx: &mut Ctx, fun: FunctionCode, addr: u16, value: u16) -> ModbusResult
{
    let mut data = vec![];
    data.write_u16::<BigEndian>(value).unwrap();
    let pack = Packet::new(ctx, fun, addr).encode_with_data(&mut data);
    println!("{:?}", pack);
    match ctx.stream.write_all(&pack) {
        Ok(_)  => {
            let mut in_buf = Vec::new();
            ctx.stream.read_to_end(&mut in_buf);
            Ok(ExceptionCode::Acknowledge)
        }
        Err(e) => panic!("Error sending data: {}", e)
    }

}

pub fn write_single_coil(ctx: &mut Ctx, addr: u16, v: BitValue) -> ModbusResult
{
    write_single(ctx, FunctionCode::WriteSingleCoil, addr, v as u16)
}

#[cfg(test)]
fn start_dummy_server(port: &str) -> Result<ChildKiller> {
    Ok(ChildKiller(Command::new("./test/diagslave")
                        .arg("-m").arg("tcp")
                        .arg("-p").arg(port)
                        .stdout(Stdio::null())
                        .spawn()
                        .unwrap_or_else(|e| { panic!("failed to execute process: {}", e) })))
}

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
    thread::sleep_ms(500);
    let mut ctx = Ctx::new_with_port("127.0.0.1", 2222).unwrap();
    let mut hd = Packet::new(&mut ctx, FunctionCode::ReadCoils, 0);
    assert!(hd.tid == 1u16);
    hd = Packet::new(&mut ctx, FunctionCode::ReadCoils, 0);
    assert!(hd.tid == 2u16);
    ctx.tid = u16::MAX;
    hd = Packet::new(&mut ctx, FunctionCode::ReadCoils, 0);
    assert!(hd.tid == 0);
}

#[test]
fn test_write_single_coil() {
    let _s = start_dummy_server("2223");
    thread::sleep_ms(500);
    let mut ctx = Ctx::new_with_port("127.0.0.1", 2223).unwrap();
    assert!(write_single_coil(&mut ctx, 0, BitValue::On).is_ok());
}
