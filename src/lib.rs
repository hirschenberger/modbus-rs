#[macro_use]
extern crate enum_primitive;
extern crate num;
extern crate rustc_serialize;
extern crate bincode;
extern crate byteorder;

pub mod tcp;

enum_from_primitive! {
#[derive(Debug, PartialEq)]
/// Modbus function codes
pub enum FunctionCode {
    ReadCoils               = 0x01,
    ReadDiscreteInputs      = 0x02,
    ReadHoldingRegisters    = 0x03,
    ReadInputRegisters      = 0x04,
    WriteSingleCoil         = 0x05,
    WriteSingleRegister     = 0x06,
    ReadExceptionStatus     = 0x07,
    WriteMultipleCoils      = 0x0f,
    WriteMultipleRegisters  = 0x10,
    ReportSlaveId           = 0x11,
    MaskWriteRegister       = 0x16,
    WriteAndReadRegisters   = 0x17
}
}

enum_from_primitive! {
#[derive(Debug, PartialEq)]
/// Modbus exception codes
pub enum ExceptionCode {
    IllegalFunction     = 0x01,
    IllegalDataAddress,
    IllagalDataValue,
    SlaveOrServerFailure,
    Acknowledge,
    SlaveOrServerBusy,
    NegativeAcknowledge,
    MemoryParity,
    NotDefined,
    GatewayPath,
    GatewayTarget
}
}

pub type ModbusResult = std::result::Result<ExceptionCode, ExceptionCode>;

enum_from_primitive! {
#[derive(Debug, PartialEq)]
/// Modbus function codes
pub enum BitValue {
    On  = 0xff00,
    Off = 0x0000
}
}
