use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use enum_primitive::FromPrimitive;
use std::borrow::BorrowMut;
use std::io::{self, Cursor, Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::time::Duration;

use {binary, Client, Coil, Error, ExceptionCode, Function, Reason, Result};

#[cfg(feature = "read-device-info")]
use mei;

const MODBUS_PROTOCOL_TCP: u16 = 0x0000;
const MODBUS_TCP_DEFAULT_PORT: u16 = 502;
const MODBUS_HEADER_SIZE: usize = 7;
const MODBUS_MAX_PACKET_SIZE: usize = 260;

/// Config structure for more control over the tcp socket settings
#[derive(Clone, Copy)]
pub struct Config {
    /// The TCP port to use for communication (Default: `502`)
    pub tcp_port: u16,
    /// Connection timeout for TCP socket (Default: `OS Default`)
    pub tcp_connect_timeout: Option<Duration>,
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
            tcp_connect_timeout: None,
            tcp_read_timeout: None,
            tcp_write_timeout: None,
            modbus_uid: 1,
        }
    }
}

#[derive(Debug, PartialEq)]
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

    fn pack(&self) -> Result<Vec<u8>> {
        let mut buff = vec![];
        buff.write_u16::<BigEndian>(self.tid)?;
        buff.write_u16::<BigEndian>(self.pid)?;
        buff.write_u16::<BigEndian>(self.len)?;
        buff.write_u8(self.uid)?;
        Ok(buff)
    }

    fn unpack(buff: &[u8]) -> Result<Header> {
        let mut rdr = Cursor::new(buff);
        Ok(Header {
            tid: rdr.read_u16::<BigEndian>()?,
            pid: rdr.read_u16::<BigEndian>()?,
            len: rdr.read_u16::<BigEndian>()?,
            uid: rdr.read_u8()?,
        })
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
        let stream = match cfg.tcp_connect_timeout {
            Some(timeout) => {
                // Call to connect_timeout needs to be done on a single address
                let mut socket_addrs = (addr, cfg.tcp_port).to_socket_addrs()?;
                TcpStream::connect_timeout(&socket_addrs.next().unwrap(), timeout)
            }
            None => TcpStream::connect((addr, cfg.tcp_port)),
        };

        match stream {
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

    fn read(&mut self, fun: &Function) -> Result<Vec<u8>> {
        let packed_size = |v: u16| v / 8 + if v % 8 > 0 { 1 } else { 0 };
        let (addr, count, expected_bytes) = match *fun {
            Function::ReadCoils(a, c) | Function::ReadDiscreteInputs(a, c) => {
                (a, c, packed_size(c) as usize)
            }
            Function::ReadHoldingRegisters(a, c) | Function::ReadInputRegisters(a, c) => {
                (a, c, 2 * c as usize)
            }
            _ => return Err(Error::InvalidFunction),
        };

        if count < 1 {
            return Err(Error::InvalidData(Reason::RecvBufferEmpty));
        }

        if count as usize > MODBUS_MAX_PACKET_SIZE {
            return Err(Error::InvalidData(Reason::UnexpectedReplySize));
        }

        let header = Header::new(self, MODBUS_HEADER_SIZE as u16 + 6u16);
        let mut buff = header.pack()?;
        buff.write_u8(fun.code())?;
        buff.write_u16::<BigEndian>(addr)?;
        buff.write_u16::<BigEndian>(count)?;

        match self.stream.write_all(&buff) {
            Ok(_s) => {
                let mut reply = vec![0; MODBUS_HEADER_SIZE + expected_bytes + 2];
                match self.stream.read(&mut reply) {
                    Ok(_s) => {
                        let resp_hd = Header::unpack(&reply[..MODBUS_HEADER_SIZE])?;
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
        if reply[8] as usize != expected_bytes
            || reply.len() != MODBUS_HEADER_SIZE + expected_bytes + 2
        {
            Err(Error::InvalidData(Reason::UnexpectedReplySize))
        } else {
            let mut d = Vec::new();
            d.extend_from_slice(&reply[MODBUS_HEADER_SIZE + 2..]);
            Ok(d)
        }
    }

    fn write_single(&mut self, fun: &Function) -> Result<()> {
        let (addr, value) = match *fun {
            Function::WriteSingleCoil(a, v) | Function::WriteSingleRegister(a, v) => (a, v),
            _ => return Err(Error::InvalidFunction),
        };

        let mut buff = vec![0; MODBUS_HEADER_SIZE]; // Header gets filled in later
        buff.write_u8(fun.code())?;
        buff.write_u16::<BigEndian>(addr)?;
        buff.write_u16::<BigEndian>(value)?;
        self.write(&mut buff)
    }

    fn write_multiple(&mut self, fun: &Function) -> Result<()> {
        let (addr, quantity, values) = match *fun {
            Function::WriteMultipleCoils(a, q, v) | Function::WriteMultipleRegisters(a, q, v) => {
                (a, q, v)
            }
            _ => return Err(Error::InvalidFunction),
        };

        let mut buff = vec![0; MODBUS_HEADER_SIZE]; // Header gets filled in later
        buff.write_u8(fun.code())?;
        buff.write_u16::<BigEndian>(addr)?;
        buff.write_u16::<BigEndian>(quantity)?;
        buff.write_u8(values.len() as u8)?;
        for v in values {
            buff.write_u8(*v)?;
        }
        self.write(&mut buff)
    }

    fn write(&mut self, buff: &mut [u8]) -> Result<()> {
        if buff.is_empty() {
            return Err(Error::InvalidData(Reason::SendBufferEmpty));
        }

        if buff.len() > MODBUS_MAX_PACKET_SIZE {
            return Err(Error::InvalidData(Reason::SendBufferTooBig));
        }

        let header = Header::new(self, buff.len() as u16 + 1u16);
        let head_buff = header.pack()?;
        {
            let mut start = Cursor::new(buff.borrow_mut());
            start.write_all(&head_buff)?;
        }
        match self.stream.write_all(buff) {
            Ok(_s) => {
                let reply = &mut [0; 12];
                match self.stream.read(reply) {
                    Ok(_s) => {
                        let resp_hd = Header::unpack(reply)?;
                        Transport::validate_response_header(&header, &resp_hd)?;
                        Transport::validate_response_code(buff, reply)
                    }
                    Err(e) => Err(Error::Io(e)),
                }
            }
            Err(e) => Err(Error::Io(e)),
        }
    }

    pub fn close(&mut self) -> Result<()> {
        self.stream.shutdown(Shutdown::Both).map_err(Error::Io)
    }

    pub fn try_clone(&self) -> Result<Self> {
        Ok(Self {
            tid: self.tid,
            uid: self.uid,
            stream: self.stream.try_clone()?,
        })
    }

    #[cfg(feature = "read-device-info")]
    /**
    Some devices support modbus function 43 (Modbus Encasulated Interface) to read device information as strings.
    This will return an `IllegalFunction (0x01)` exception code if this request is not supported by the device.
    */
    pub fn read_device_info(
        &mut self,
        obj_category: mei::DeviceInfoCategory,
    ) -> Result<Vec<mei::DeviceInfoObject>> {
        let mut info: Vec<mei::DeviceInfoObject> = vec![];
        let mut buff = vec![0; MODBUS_HEADER_SIZE]; // Header gets filled in later
        buff.write_u8(0x2B)?; // Modbus Encapsulated Interface (Function code 43)
        buff.write_u8(0x0E)?; // MEI Type 14 (Read Device Identification)
        buff.write_u8(match obj_category {
            mei::DeviceInfoCategory::Basic => 0x01,
            mei::DeviceInfoCategory::Regular => 0x02,
            mei::DeviceInfoCategory::Extended => 0x03,
        })?;
        buff.write_u8(0x00)?; // Object ID

        let header = Header::new(self, buff.len() as u16 + 1u16);
        let head_buff = header.pack()?;
        {
            let mut start: Cursor<&mut Vec<u8>> = Cursor::new(buff.borrow_mut());
            start.write_all(&head_buff)?;
        }
        match self.stream.write_all(&buff) {
            Ok(_s) => {
                let reply = &mut [0; MODBUS_MAX_PACKET_SIZE];
                match self.stream.read(reply) {
                    Ok(_s) => {
                        let resp_hd = Header::unpack(reply)?;
                        Transport::validate_response_header(&header, &resp_hd)?;
                        Transport::validate_response_code(&buff, reply)?;

                        let resp_body = reply[7..(6 + resp_hd.len) as usize].to_vec();
                        let obj_count = resp_body[6] as usize;
                        let mut cursor: usize = 6;
                        for _ in 0..obj_count {
                            cursor += 1;
                            let id = resp_body[cursor];

                            cursor += 1;
                            let len = resp_body[cursor] as usize;

                            let mut val_buf: Vec<u8> = vec![];
                            for _ in 0..len {
                                cursor += 1;
                                val_buf.push(resp_body[cursor])
                            }

                            let object = mei::DeviceInfoObject::new(
                                id,
                                match String::from_utf8(val_buf) {
                                    Ok(val) => val,
                                    Err(_) => return Err(Error::ParseInfoError),
                                },
                            );
                            info.push(object)
                        }
                        Ok(())
                    }
                    Err(e) => Err(Error::Io(e)),
                }
            }
            Err(e) => Err(Error::Io(e)),
        }?;
        Ok(info)
    }
}

impl Client for Transport {
    /// Read `count` bits starting at address `addr`.
    fn read_coils(&mut self, addr: u16, count: u16) -> Result<Vec<Coil>> {
        let bytes = self.read(&Function::ReadCoils(addr, count))?;
        Ok(binary::unpack_bits(&bytes, count))
    }

    /// Read `count` input bits starting at address `addr`.
    fn read_discrete_inputs(&mut self, addr: u16, count: u16) -> Result<Vec<Coil>> {
        let bytes = self.read(&Function::ReadDiscreteInputs(addr, count))?;
        Ok(binary::unpack_bits(&bytes, count))
    }

    /// Read `count` 16bit registers starting at address `addr`.
    fn read_holding_registers(&mut self, addr: u16, count: u16) -> Result<Vec<u16>> {
        let bytes = self.read(&Function::ReadHoldingRegisters(addr, count))?;
        binary::pack_bytes(&bytes[..])
    }

    /// Read `count` 16bit input registers starting at address `addr`.
    fn read_input_registers(&mut self, addr: u16, count: u16) -> Result<Vec<u16>> {
        let bytes = self.read(&Function::ReadInputRegisters(addr, count))?;
        binary::pack_bytes(&bytes[..])
    }

    /// Write a single coil (bit) to address `addr`.
    fn write_single_coil(&mut self, addr: u16, value: Coil) -> Result<()> {
        self.write_single(&Function::WriteSingleCoil(addr, value.code()))
    }

    /// Write a single 16bit register to address `addr`.
    fn write_single_register(&mut self, addr: u16, value: u16) -> Result<()> {
        self.write_single(&Function::WriteSingleRegister(addr, value))
    }

    /// Write a multiple coils (bits) starting at address `addr`.
    fn write_multiple_coils(&mut self, addr: u16, values: &[Coil]) -> Result<()> {
        let bytes = binary::pack_bits(values);
        self.write_multiple(&Function::WriteMultipleCoils(
            addr,
            values.len() as u16,
            &bytes,
        ))
    }

    /// Write a multiple 16bit registers starting at address `addr`.
    fn write_multiple_registers(&mut self, addr: u16, values: &[u16]) -> Result<()> {
        let bytes = binary::unpack_bytes(values);
        self.write_multiple(&Function::WriteMultipleRegisters(
            addr,
            values.len() as u16,
            &bytes,
        ))
    }

    /// Set the unit identifier.
    fn set_uid(&mut self, uid: u8) {
        self.uid = uid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread;
    #[test]
    fn serialize_header() {
        let header = Header {
            tid: 12816,
            pid: 3930,
            len: 99,
            uid: 68,
        };
        let serialized = header.pack().unwrap();
        let deserialized = Header::unpack(&vec![50, 16, 15, 90, 0, 99, 68]).unwrap();
        let re_deserialized = Header::unpack(&serialized).unwrap();
        assert_eq!(serialized, vec![50, 16, 15, 90, 0, 99, 68]);
        assert_eq!(deserialized, header);
        assert_eq!(re_deserialized, header);
    }
    #[test]
    fn try_clone() {
        use std::sync::atomic::{AtomicBool, Ordering};

        static STARTED: AtomicBool = AtomicBool::new(false);
        static CLOSED: AtomicBool = AtomicBool::new(false);

        let jh = thread::spawn(|| {
            let listener = TcpListener::bind("localhost:34254").unwrap();
            STARTED.store(true, Ordering::Relaxed);
            listener
                .accept()
                .and_then(|_| {
                    while !CLOSED.load(Ordering::Relaxed) {}
                    Ok(())
                })
                .unwrap();
        });

        while !STARTED.load(Ordering::Relaxed) {}

        let new_stream = TcpStream::connect("localhost:34254").unwrap();
        let mut transport = Transport {
            tid: 1,
            uid: 2,
            stream: new_stream,
        };

        match transport.try_clone() {
            Ok(mut cl) => {
                assert_eq!(cl.tid, transport.tid);
                assert_eq!(cl.uid, transport.uid);
                assert_eq!(
                    cl.stream.local_addr().unwrap(),
                    transport.stream.local_addr().unwrap()
                );
                assert_eq!(
                    cl.stream.peer_addr().unwrap(),
                    transport.stream.peer_addr().unwrap()
                );
                cl.close().expect("unable to close TcpStream clone");
                assert!(transport.stream.write(b"data").is_err());
            }
            Err(_) => panic!("failed to clone"),
        };

        CLOSED.store(true, Ordering::Relaxed);
        jh.join().unwrap();
    }
}
