use std::u16;
use num::{FromPrimitive, ToPrimitive};

const PROTOCOL_MODBUS_TCP: u16 = 0x0000;

enum_from_primitive! {
#[derive(Debug, PartialEq)]
/// Modbus FunctionCodes
pub enum FunctionCode {
    ReadCoils               = 0x01,
    ReadDiscreteInputs      = 0x02
}
}

/// Context object which holds state for all modbus operations.
pub struct Ctx {
    tid: u16
}

impl Ctx {
    /// Create a new context `Ctx` context object.
    pub fn new() -> Ctx {
        Ctx { tid: 0 }
    }

    /// Create a new transaction Id, incrementing the previous one.
    ///
    /// The Id is wrapping around if the Id reaches `u16::MAX`.
    pub fn new_tid(self: &mut Self) -> u16 {
        // wrap around or increment
        if self.tid  < u16::MAX {
            self.tid += 1u16;
        } else {
            self.tid = 0u16;
        }
        self.tid
    }
}

pub struct Header {
    tid: u16,
    pid: u16,
    len: u16,
    uid: u8,
    fun: u8
}

impl Header {
    pub fn new(ctx: &mut Ctx, uid: u8, fun: FunctionCode) -> Header {
        Header {
            tid: ctx.new_tid(),
            pid: PROTOCOL_MODBUS_TCP,
            len: 0,
            uid: uid,
            fun: fun as u8
        }
    }
}

#[test]
fn header_tid_creation() {
    let mut ctx = Ctx::new();
    let mut hd = Header::new(&mut ctx, 0, FunctionCode::ReadCoils);
    assert!(hd.tid == 1);
    hd = Header::new(&mut ctx, 0, FunctionCode::ReadCoils);
    assert!(hd.tid == 2);
    ctx.tid = u16::MAX;
    hd = Header::new(&mut ctx, 0, FunctionCode::ReadCoils);
    assert!(hd.tid == 0);
}
