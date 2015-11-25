use {Result, BitValue};

pub trait Client {
    fn read_discrete_inputs(&mut self, address: u16, quantity: u16) -> Result<Vec<BitValue>>;

    fn read_coils(&mut self, address: u16, quantity: u16) -> Result<Vec<BitValue>>;

    fn write_single_coil(&mut self, address: u16, value: BitValue) -> Result<()>;

    fn write_multiple_coils(&mut self, address: u16, coils: &[BitValue]) -> Result<()>;

    fn read_input_registers(&mut self, address: u16, quantity: u16) -> Result<Vec<u16>>;

    fn read_holding_registers(&mut self, address: u16, quantity: u16) -> Result<Vec<u16>>;

    fn write_single_register(&mut self, address: u16, value: u16) -> Result<()>;

    fn write_multiple_registers(&mut self, address: u16, values: &[u16]) -> Result<()>;
}
