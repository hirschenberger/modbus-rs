use {Coil, Transport, Client, Result};

pub enum CoilDropFunction {
    On,
    Off,
    Toggle,
}

pub enum RegisterDropFunction<'a> {
    Zero,
    Increment,
    Decrement,
    Value(u16),
    Fun(&'a Fn(u16) -> u16),
}

pub struct ScopedCoil<'a> {
    pub address: u16,
    pub fun: CoilDropFunction,
    pub transport: &'a mut Transport,
}

impl<'a> Drop for ScopedCoil<'a> {
    fn drop(&mut self) {
        // assume everything works as expected, we can't return an error
        let value = self.transport.read_coils(self.address, 1).unwrap()[0];
        let drop_value = match self.fun {
            CoilDropFunction::On => Coil::On,
            CoilDropFunction::Off => Coil::Off,
            CoilDropFunction::Toggle => if value == Coil::On {
                Coil::Off
            } else {
                Coil::On
            },
        };
        self.transport.write_single_coil(self.address, drop_value).unwrap()
    }
}

impl<'a> ScopedCoil<'a> {
    pub fn new(transport: &mut Transport,
               address: u16,
               fun: CoilDropFunction)
               -> Result<ScopedCoil> {
        Ok(ScopedCoil {
            address: address,
            fun: fun,
            transport: transport,
        })
    }

    pub fn mut_transport<'b>(&'b mut self) -> &'b mut Transport {
        self.transport
    }
}

pub struct ScopedRegister<'a> {
    pub address: u16,
    pub fun: RegisterDropFunction<'a>,
    pub transport: &'a mut Transport,
}

impl<'a> Drop for ScopedRegister<'a> {
    fn drop(&mut self) {
        // assume everything works as expected, we can't return an error
        let value = self.transport.read_holding_registers(self.address, 1).unwrap()[0];
        let drop_value = match self.fun {
            RegisterDropFunction::Zero => 0u16,
            RegisterDropFunction::Increment => value + 1,
            RegisterDropFunction::Decrement => value - 1,
            RegisterDropFunction::Value(v) => v,
            RegisterDropFunction::Fun(f) => f(value),
        };
        self.transport.write_single_register(self.address, drop_value).unwrap()
    }
}

impl<'a> ScopedRegister<'a> {
    pub fn new<'b>(transport: &'b mut Transport,
                   address: u16,
                   fun: RegisterDropFunction<'b>)
                   -> Result<ScopedRegister<'b>> {
        Ok(ScopedRegister {
            address: address,
            fun: fun,
            transport: transport,
        })
    }

    pub fn mut_transport<'b>(&'b mut self) -> &'b mut Transport {
        self.transport
    }
}
