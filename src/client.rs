use crate::{Coil, Result};

pub trait Client {
    fn read_discrete_inputs(&mut self, address: &str, quantity: u16) -> Result<Vec<Coil>>;

    fn read_coils(&mut self, address: &str, quantity: u16) -> Result<Vec<Coil>>;

    fn write_single_coil(&mut self, address: &str, value: Coil) -> Result<()>;

    fn write_multiple_coils(&mut self, address: &str, coils: &[Coil]) -> Result<()>;

    fn read_input_registers(&mut self, address: &str, quantity: u16) -> Result<Vec<u16>>;

    fn read_holding_registers(&mut self, address: &str, quantity: u16) -> Result<Vec<u16>>;

    fn write_single_register(&mut self, address: &str, value: u16) -> Result<()>;

    fn write_multiple_registers(&mut self, address: &str, values: &[u16]) -> Result<()>;

    fn write_read_multiple_registers(
        &mut self,
        write_address: &str,
        write_quantity: u16,
        write_values: &[u16],
        read_address: &str,
        read_quantity: u16,
    ) -> Result<Vec<u16>>;

    fn set_uid(&mut self, uid: u8);
}
