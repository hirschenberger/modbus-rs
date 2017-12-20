extern crate test_server;
extern crate modbus;

mod connection_tests {
    use modbus::tcp::{Config, Transport};
    use std::time::{Duration, Instant};

    #[test]
    fn test_connect_timeout() {
        let mut cfg = Config::default();
        cfg.tcp_connect_timeout = Some(Duration::from_millis(1000));
        let now = Instant::now();
        if Transport::new_with_cfg("30.30.30.30", cfg).is_err() {
            let elapsed = now.elapsed().as_secs();
            assert_eq!(elapsed, 1, "Elapsed: {}", elapsed);
        }
    }
}

#[cfg(feature="modbus-server-tests")]
mod modbus_server_tests {
    use test_server::{ChildKiller, start_dummy_server};
    use modbus::tcp::{Config, Transport};
    use modbus::{Client, Coil};
    use modbus::scoped::{ScopedCoil, ScopedRegister, CoilDropFunction, RegisterDropFunction};

    fn start_dummy_server_with_cfg() -> (ChildKiller, Config) {
        let (k, port) = start_dummy_server(None);
        let mut cfg = Config::default();
        cfg.tcp_port = port;
        (k, cfg)
    }

    /// /////////////////////
    /// simple READ tests
    #[test]
    fn test_read_coils() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert_eq!(trans.read_coils(0, 5).unwrap().len(), 5);
        assert!(trans.read_coils(0, 5).unwrap().iter().all(|c| *c == Coil::Off));
    }

    #[test]
    fn test_read_discrete_inputs() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert_eq!(trans.read_discrete_inputs(0, 5).unwrap().len(), 5);
        assert!(trans.read_discrete_inputs(0, 5).unwrap().iter().all(|c| *c == Coil::Off));
    }

    #[test]
    fn test_read_holding_registers() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert_eq!(trans.read_holding_registers(0, 5).unwrap().len(), 5);
        assert!(trans.read_holding_registers(0, 5).unwrap().iter().all(|c| *c == 0));
    }

    #[test]
    fn test_read_input_registers() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert_eq!(trans.read_input_registers(0, 5).unwrap().len(), 5);
        assert!(trans.read_input_registers(0, 5).unwrap().iter().all(|c| *c == 0));
    }

    /// /////////////////////
    /// simple WRITE tests
    #[test]
    fn test_write_single_coil() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert!(trans.write_single_coil(0, Coil::On).is_ok());
    }

    #[test]
    fn test_write_single_register() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert!(trans.write_single_register(0, 1).is_ok());
    }

    #[test]
    fn test_write_multiple_coils() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert!(trans.write_multiple_coils(0, &[Coil::On, Coil::Off]).is_ok());
        // assert!(write_multiple_coils(&mut trans, 0, &[]).is_err());
    }

    #[test]
    fn test_write_multiple_registers() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert!(trans.write_multiple_registers(0, &[0, 1, 2, 3]).is_ok());
        // assert!(write_multiple_registers(&mut trans, 0, &[]).is_err());
    }

    /// /////////////////////
    /// coil WRITE-READ tests
    #[test]
    fn test_write_read_single_coils() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();

        assert!(trans.write_single_coil(1, Coil::On).is_ok());
        assert!(trans.write_single_coil(3, Coil::On).is_ok());
        assert_eq!(trans.read_coils(0, 5).unwrap(),
                   vec![Coil::Off, Coil::On, Coil::Off, Coil::On, Coil::Off]);
        assert_eq!(trans.read_coils(1, 5).unwrap(),
                   vec![Coil::On, Coil::Off, Coil::On, Coil::Off, Coil::Off]);
        assert!(trans.write_single_coil(10, Coil::On).is_ok());
        assert!(trans.write_single_coil(11, Coil::On).is_ok());
        assert_eq!(trans.read_coils(9, 4).unwrap(),
                   vec![Coil::Off, Coil::On, Coil::On, Coil::Off]);
        assert!(trans.write_single_coil(10, Coil::Off).is_ok());
        assert!(trans.write_single_coil(11, Coil::Off).is_ok());
        assert_eq!(trans.read_coils(9, 4).unwrap(),
                   vec![Coil::Off, Coil::Off, Coil::Off, Coil::Off]);
    }

    #[test]
    fn test_write_read_single_register() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert!(trans.write_single_register(0, 23).is_ok());
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), vec![23]);
        assert!(trans.write_single_register(0, 0).is_ok());
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), vec![0]);
        assert_eq!(trans.read_input_registers(0, 1).unwrap(), vec![0]);
        assert!(trans.write_single_register(0, 23).is_ok());
        assert!(trans.write_single_register(1, 24).is_ok());
        assert_eq!(trans.read_holding_registers(0, 2).unwrap(), vec![23, 24]);
    }

    #[test]
    fn test_write_read_multiple_coils() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert!(trans.write_multiple_coils(0, &[Coil::Off, Coil::On]).is_ok());
        assert_eq!(trans.read_coils(0, 3).unwrap(),
                   &[Coil::Off, Coil::On, Coil::Off]);
        assert!(trans.write_multiple_coils(0, &[Coil::On; 9]).is_ok());
        assert_eq!(trans.read_coils(0, 9).unwrap(), &[Coil::On; 9]);
    }

    #[test]
    fn test_write_read_multiple_registers() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        // assert!(write_multiple_registers(&mut trans, 0, &[]).is_err());
        assert!(trans.write_multiple_registers(0, &[23]).is_ok());
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), &[23]);
        assert!(trans.write_multiple_registers(0, &[1, 2, 3]).is_ok());
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), &[1]);
        assert_eq!(trans.read_holding_registers(1, 1).unwrap(), &[2]);
        assert_eq!(trans.read_holding_registers(2, 1).unwrap(), &[3]);
        assert_eq!(trans.read_holding_registers(0, 3).unwrap(), &[1, 2, 3]);
    }

    #[test]
    fn test_write_too_big() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();
        assert!(trans.write_multiple_registers(0, &[0xdead; 123]).is_ok());
        assert!(trans.write_multiple_registers(0, &[0xdead; 124]).is_err());
    }

    #[test]
    fn test_scoped_coil() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();

        {
            let mut auto = ScopedCoil::new(&mut trans, 0, CoilDropFunction::On).unwrap();
            assert_eq!(auto.mut_transport().read_coils(0, 1).unwrap(),
                       vec![Coil::Off]);
        }
        assert_eq!(trans.read_coils(0, 1).unwrap(), vec![Coil::On]);

        {
            let mut auto = ScopedCoil::new(&mut trans, 0, CoilDropFunction::Off).unwrap();
            assert_eq!(auto.mut_transport().read_coils(0, 1).unwrap(),
                       vec![Coil::On]);
        }
        assert_eq!(trans.read_coils(0, 1).unwrap(), vec![Coil::Off]);

        {
            let mut auto = ScopedCoil::new(&mut trans, 0, CoilDropFunction::Toggle).unwrap();
            assert_eq!(auto.mut_transport().read_coils(0, 1).unwrap(),
                       vec![Coil::Off]);
        }
        assert_eq!(trans.read_coils(0, 1).unwrap(), vec![Coil::On]);

        {
            let mut auto = ScopedCoil::new(&mut trans, 0, CoilDropFunction::Toggle).unwrap();
            assert_eq!(auto.mut_transport().read_coils(0, 1).unwrap(),
                       vec![Coil::On]);
        }
        assert_eq!(trans.read_coils(0, 1).unwrap(), vec![Coil::Off]);

        // coil address 1
        {
            let mut auto = ScopedCoil::new(&mut trans, 1, CoilDropFunction::Toggle).unwrap();
            assert_eq!(auto.mut_transport().read_coils(1, 1).unwrap(),
                       vec![Coil::Off]);
        }
        assert_eq!(trans.read_coils(1, 1).unwrap(), vec![Coil::On]);

    }

    #[test]
    fn test_scoped_register() {
        let (_s, cfg) = start_dummy_server_with_cfg();
        let mut trans = Transport::new_with_cfg("127.0.0.1", cfg).unwrap();

        {
            let mut auto = ScopedRegister::new(&mut trans, 0, RegisterDropFunction::Value(0xbeef))
                .unwrap();
            assert_eq!(auto.mut_transport().read_holding_registers(0, 1).unwrap(),
                       vec![0x0000]);
        }
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), vec![0xbeef]);

        {
            let mut auto = ScopedRegister::new(&mut trans, 0, RegisterDropFunction::Zero).unwrap();
            assert_eq!(auto.mut_transport().read_holding_registers(0, 1).unwrap(),
                       vec![0xbeef]);
        }
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), vec![0x0000]);

        {
            let mut auto = ScopedRegister::new(&mut trans, 0, RegisterDropFunction::Increment)
                .unwrap();
            assert_eq!(auto.mut_transport().read_holding_registers(0, 1).unwrap(),
                       vec![0x0000]);
        }
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), vec![0x0001]);

        {
            let mut auto = ScopedRegister::new(&mut trans, 0, RegisterDropFunction::Increment)
                .unwrap();
            assert_eq!(auto.mut_transport().read_holding_registers(0, 1).unwrap(),
                       vec![0x0001]);
        }
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), vec![0x0002]);

        {
            let mut auto = ScopedRegister::new(&mut trans, 0, RegisterDropFunction::Decrement)
                .unwrap();
            assert_eq!(auto.mut_transport().read_holding_registers(0, 1).unwrap(),
                       vec![0x0002]);
        }
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), vec![0x0001]);

        {
            let fun = |v| v + 0xbeee;
            let mut auto = ScopedRegister::new(&mut trans, 0, RegisterDropFunction::Fun(&fun))
                .unwrap();
            assert_eq!(auto.mut_transport().read_holding_registers(0, 1).unwrap(),
                       vec![0x0001]);
        }
        assert_eq!(trans.read_holding_registers(0, 1).unwrap(), vec![0xbeef]);


    }
}
