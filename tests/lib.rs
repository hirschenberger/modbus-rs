extern crate test_server;
extern crate modbus;

#[cfg(feature="modbus-server-tests")]
mod modbus_server_tests {
    use test_server::start_dummy_server;
    use modbus::tcp::Ctx;
    use modbus::{Client, BitValue};

    /// /////////////////////
    /// simple READ tests
    #[test]
    fn test_read_coils() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert_eq!(ctx.read_coils(0, 5).unwrap().len(), 5);
        assert!(ctx.read_coils(0, 5).unwrap().iter().all(|c| *c == BitValue::Off));
    }

    #[test]
    fn test_read_discrete_inputs() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert_eq!(ctx.read_discrete_inputs(0, 5).unwrap().len(), 5);
        assert!(ctx.read_discrete_inputs(0, 5).unwrap().iter().all(|c| *c == BitValue::Off));
    }

    #[test]
    fn test_read_holding_registers() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert_eq!(ctx.read_holding_registers(0, 5).unwrap().len(), 5);
        assert!(ctx.read_holding_registers(0, 5).unwrap().iter().all(|c| *c == 0));
    }

    #[test]
    fn test_read_input_registers() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert_eq!(ctx.read_input_registers(0, 5).unwrap().len(), 5);
        assert!(ctx.read_input_registers(0, 5).unwrap().iter().all(|c| *c == 0));
    }

    /// /////////////////////
    /// simple WRITE tests
    #[test]
    fn test_write_single_coil() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert!(ctx.write_single_coil(0, BitValue::On).is_ok());
    }

    #[test]
    fn test_write_single_register() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert!(ctx.write_single_register(0, 1).is_ok());
    }

    #[test]
    fn test_write_multiple_coils() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert!(ctx.write_multiple_coils(0, &[BitValue::On, BitValue::Off]).is_ok());
        // assert!(write_multiple_coils(&mut ctx, 0, &[]).is_err());
    }

    #[test]
    fn test_write_multiple_registers() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert!(ctx.write_multiple_registers(0, &[0, 1, 2, 3]).is_ok());
        // assert!(write_multiple_registers(&mut ctx, 0, &[]).is_err());
    }

    /// /////////////////////
    /// coil WRITE-READ tests
    #[test]
    fn test_write_read_single_coils() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();

        assert!(ctx.write_single_coil(1, BitValue::On).is_ok());
        assert!(ctx.write_single_coil(3, BitValue::On).is_ok());
        assert_eq!(ctx.read_coils(0, 5).unwrap(),
                   vec![BitValue::Off, BitValue::On, BitValue::Off, BitValue::On, BitValue::Off]);
        assert_eq!(ctx.read_coils(1, 5).unwrap(),
                   vec![BitValue::On, BitValue::Off, BitValue::On, BitValue::Off, BitValue::Off]);
        assert!(ctx.write_single_coil(10, BitValue::On).is_ok());
        assert!(ctx.write_single_coil(11, BitValue::On).is_ok());
        assert_eq!(ctx.read_coils(9, 4).unwrap(),
                   vec![BitValue::Off, BitValue::On, BitValue::On, BitValue::Off]);
        assert!(ctx.write_single_coil(10, BitValue::Off).is_ok());
        assert!(ctx.write_single_coil(11, BitValue::Off).is_ok());
        assert_eq!(ctx.read_coils(9, 4).unwrap(),
                   vec![BitValue::Off, BitValue::Off, BitValue::Off, BitValue::Off]);
    }

    #[test]
    fn test_write_read_single_register() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert!(ctx.write_single_register(0, 23).is_ok());
        assert_eq!(ctx.read_holding_registers(0, 1).unwrap(), vec![23]);
        assert!(ctx.write_single_register(0, 0).is_ok());
        assert_eq!(ctx.read_holding_registers(0, 1).unwrap(), vec![0]);
        assert_eq!(ctx.read_input_registers(0, 1).unwrap(), vec![0]);
        assert!(ctx.write_single_register(0, 23).is_ok());
        assert!(ctx.write_single_register(1, 24).is_ok());
        assert_eq!(ctx.read_holding_registers(0, 2).unwrap(), vec![23, 24]);
    }

    #[test]
    fn test_write_read_multiple_coils() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        assert!(ctx.write_multiple_coils(0, &[BitValue::Off, BitValue::On]).is_ok());
        assert_eq!(ctx.read_coils(0, 3).unwrap(),
                   &[BitValue::Off, BitValue::On, BitValue::Off]);
        assert!(ctx.write_multiple_coils(0, &[BitValue::On; 9]).is_ok());
        assert_eq!(ctx.read_coils(0, 9).unwrap(), &[BitValue::On; 9]);
    }

    #[test]
    fn test_write_read_multiple_registers() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        // assert!(write_multiple_registers(&mut ctx, 0, &[]).is_err());
        assert!(ctx.write_multiple_registers(0, &[23]).is_ok());
        assert_eq!(ctx.read_holding_registers(0, 1).unwrap(), &[23]);
        assert!(ctx.write_multiple_registers(0, &[1, 2, 3]).is_ok());
        assert_eq!(ctx.read_holding_registers(0, 1).unwrap(), &[1]);
        assert_eq!(ctx.read_holding_registers(1, 1).unwrap(), &[2]);
        assert_eq!(ctx.read_holding_registers(2, 1).unwrap(), &[3]);
        assert_eq!(ctx.read_holding_registers(0, 3).unwrap(), &[1, 2, 3]);
    }

    #[test]
    fn test_write_too_big() {
        let (_s, port) = start_dummy_server();
        let mut ctx = Ctx::new_with_port("127.0.0.1", port).unwrap();
        // (MODBUS_MAX_WRITE_COUNT - HEADER) / u16-bytes
        assert!(write_multiple_registers(&mut ctx, 0, &[0xdead; (0x79 - 12) / 2]).is_ok());
        assert!(write_multiple_registers(&mut ctx, 0, &[0xdead; (0x79 - 11) / 2]).is_err());
    }
}
