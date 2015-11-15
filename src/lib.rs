#[macro_use]
extern crate enum_primitive;
extern crate num;
extern crate rustc_serialize;
extern crate bincode;
extern crate byteorder;

pub mod tcp;

type Address  = u16;
type Quantity = u16;
type Value    = u16;

enum Function {
    ReadCoils(Address, Quantity),
    ReadDiscreteInputs(Address, Quantity),
    ReadHoldingRegisters(Address, Quantity),
    ReadInputRegisters(Address, Quantity),
    WriteSingleCoil(Address, Value),
    WriteSingleRegister(Address, Value)
}

impl Function {
    fn code(&self) -> u8 {
        match *self {
            Function::ReadCoils(_, _)             => 0x01,
            Function::ReadDiscreteInputs(_, _)    => 0x02,
            Function::ReadHoldingRegisters(_, _)  => 0x03,
            Function::ReadInputRegisters(_, _)    => 0x04,
            Function::WriteSingleCoil(_, _)       => 0x05,
            Function::WriteSingleRegister(_, _)   => 0x06
        }
    //
    // ReadExceptionStatus     = 0x07,
    // WriteMultipleCoils      = 0x0f,
    // WriteMultipleRegisters  = 0x10,
    // ReportSlaveId           = 0x11,
    // MaskWriteRegister       = 0x16,
    // WriteAndReadRegisters   = 0x17
    }
}


enum_from_primitive! {
#[derive(Debug, PartialEq)]
/// Modbus exception codes
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

pub enum IoError {
    ModbusExceptionCode(ModbusExceptionCode),
    Communication
}

pub type ModbusResult<T> = std::result::Result<T, IoError>;

enum_from_primitive! {
#[derive(Debug, PartialEq)]
/// Single bit status values
pub enum BitValue {
    On  = 0xff00,
    Off = 0x0000
}
}
