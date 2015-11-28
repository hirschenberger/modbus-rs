use {ModbusResult, Coil};
use tcp::Transport;
use std::cell::RefCell;

pub trait Client {
    fn read_discrete_inputs(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<Coil>>;

    fn read_coils(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<Coil>>;

    fn write_single_coil(&mut self, address: u16, value: Coil) -> ModbusResult<()>;

    fn write_multiple_coils(&mut self, address: u16, coils: &[Coil]) -> ModbusResult<()>;

    fn read_input_registers(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<u16>>;

    fn read_holding_registers(&mut self, address: u16, quantity: u16) -> ModbusResult<Vec<u16>>;

    fn write_single_register(&mut self, address: u16, value: u16) -> ModbusResult<()>;

    fn write_multiple_registers(&mut self, address: u16, values: &[u16]) -> ModbusResult<()>;
}

pub enum CoilDropFunction {
    On,
    Off,
    Toggle,
}

pub enum RegisterDropFunction {
    Zero,
    Value(u16),
}

pub struct ScopedCoil {
    pub address: u16,
    pub drop_value: Coil,
    pub transport: RefCell<Transport>,
}

impl Drop for ScopedCoil {
    fn drop(&mut self) {
        self.transport.borrow_mut().write_single_coil(self.address, self.drop_value).unwrap()
    }
}

impl ScopedCoil {
    fn new(transport: &mut Transport,
           address: u16,
           value: Coil,
           fun: CoilDropFunction)
           -> ModbusResult<ScopedCoil> {
        try!(transport.write_single_coil(address, value));
        let drop_value = match fun {
            CoilDropFunction::On => Coil::On,
            CoilDropFunction::Off => Coil::Off,
            CoilDropFunction::Toggle if value == Coil::On => Coil::Off,
            CoilDropFunction::Toggle if value == Coil::Off => Coil::On,
            _ => panic!("Impossible drop function"),
        };
        Ok(ScopedCoil {
            address: address,
            drop_value: drop_value,
            transport: RefCell::new(*transport),
        })
    }
}
