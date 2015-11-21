use std::io;
use std::net::TcpStream;
use std::time::Duration;
use super::Context;

const MODBUS_TCP_DEFAULT_PORT: u16 = 502;

impl Context<TcpStream> {
    /// Create a new context context object and connect it to `addr` on modbus-tcp default
    /// port (502)
    pub fn new(addr: &str) -> io::Result<Self> {
        Self::new_with_port(addr, MODBUS_TCP_DEFAULT_PORT)
    }

    /// Create a new context object and connect it to `addr` on port `port`
    pub fn new_with_port(addr: &str, port: u16) -> io::Result<Self> {
        match TcpStream::connect((addr, port)) {
            Ok(s) => {
                // set some sane tcp socket options
                let t = Duration::from_secs(5);
                try!(s.set_read_timeout(Some(t)));
                try!(s.set_write_timeout(Some(t)));
                //                try!(s.set_nodelay(true));
                //                try!(s.set_keepalive(None));
                Ok(Context {
                    tid: 0,
                    uid: 1,
                    stream: s,
                })
            }
            Err(e) => Err(e),
        }
    }
}
