use {Coil, Result};

pub trait Client {
    fn read_discrete_inputs(&mut self, address: u16, quantity: u16) -> Result<Vec<Coil>>;

    fn read_coils(&mut self, address: u16, quantity: u16) -> Result<Vec<Coil>>;

    fn write_single_coil(&mut self, address: u16, value: Coil) -> Result<()>;

    fn write_multiple_coils(&mut self, address: u16, coils: &[Coil]) -> Result<()>;

    fn read_input_registers(&mut self, address: u16, quantity: u16) -> Result<Vec<u16>>;

    fn read_holding_registers(&mut self, address: u16, quantity: u16) -> Result<Vec<u16>>;

    fn write_single_register(&mut self, address: u16, value: u16) -> Result<()>;

    fn write_multiple_registers(&mut self, address: u16, values: &[u16]) -> Result<()>;

    fn set_uid(&mut self, uid: u8);
}
