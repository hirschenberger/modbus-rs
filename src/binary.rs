use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;
use {Coil, Error, Reason, Result};

pub fn unpack_bits(bytes: &[u8], count: u16) -> Vec<Coil> {
    let mut res = Vec::with_capacity(count as usize);
    for i in 0..count {
        if (bytes[(i / 8u16) as usize] >> (i % 8)) & 0b1 > 0 {
            res.push(Coil::On);
        } else {
            res.push(Coil::Off);
        }
    }
    res
}

pub fn pack_bits(bits: &[Coil]) -> Vec<u8> {
    let bitcount = bits.len();
    let packed_size = bitcount / 8 + if bitcount % 8 > 0 { 1 } else { 0 };
    let mut res = vec![0; packed_size];
    for (i, b) in bits.iter().enumerate() {
        let v = match *b {
            Coil::On => 1u8,
            Coil::Off => 0u8,
        };
        res[(i / 8) as usize] |= v << (i % 8);
    }
    res
}

pub fn unpack_bytes(data: &[u16]) -> Vec<u8> {
    let size = data.len();
    let mut res = Vec::with_capacity(size * 2);
    for b in data {
        res.push((*b >> 8 & 0xff) as u8);
        res.push((*b & 0xff) as u8);
    }
    res
}

pub fn pack_bytes(bytes: &[u8]) -> Result<Vec<u16>> {
    let size = bytes.len();
    // check if we can create u16s from bytes by packing two u8s together without rest
    if size % 2 != 0 {
        return Err(Error::InvalidData(Reason::BytecountNotEven));
    }

    let mut res = Vec::with_capacity(size / 2 + 1);
    let mut rdr = Cursor::new(bytes);
    for _ in 0..size / 2 {
        res.push(rdr.read_u16::<BigEndian>()?);
    }
    Ok(res)
}

#[test]
fn test_unpack_bits() {
    // assert_eq!(unpack_bits(, 0), &[]);
    assert_eq!(unpack_bits(&[0, 0], 0), &[]);
    assert_eq!(unpack_bits(&[0b1], 1), &[Coil::On]);
    assert_eq!(unpack_bits(&[0b01], 2), &[Coil::On, Coil::Off]);
    assert_eq!(unpack_bits(&[0b10], 2), &[Coil::Off, Coil::On]);
    assert_eq!(unpack_bits(&[0b101], 3), &[Coil::On, Coil::Off, Coil::On]);
    assert_eq!(unpack_bits(&[0xff, 0b11], 10), &[Coil::On; 10]);
}

#[test]
fn test_pack_bits() {
    assert_eq!(pack_bits(&[]), &[]);
    assert_eq!(pack_bits(&[Coil::On]), &[1]);
    assert_eq!(pack_bits(&[Coil::Off]), &[0]);
    assert_eq!(pack_bits(&[Coil::On, Coil::Off]), &[1]);
    assert_eq!(pack_bits(&[Coil::Off, Coil::On]), &[2]);
    assert_eq!(pack_bits(&[Coil::On, Coil::On]), &[3]);
    assert_eq!(pack_bits(&[Coil::On; 8]), &[255]);
    assert_eq!(pack_bits(&[Coil::On; 9]), &[255, 1]);
    assert_eq!(pack_bits(&[Coil::Off; 8]), &[0]);
    assert_eq!(pack_bits(&[Coil::Off; 9]), &[0, 0]);
}

#[test]
fn test_unpack_bytes() {
    assert_eq!(unpack_bytes(&[]), &[]);
    assert_eq!(unpack_bytes(&[0]), &[0, 0]);
    assert_eq!(unpack_bytes(&[1]), &[0, 1]);
    assert_eq!(unpack_bytes(&[0xffff]), &[0xff, 0xff]);
    assert_eq!(unpack_bytes(&[0xffff, 0x0001]), &[0xff, 0xff, 0x00, 0x01]);
    assert_eq!(unpack_bytes(&[0xffff, 0x1001]), &[0xff, 0xff, 0x10, 0x01]);
}

#[test]
fn test_pack_bytes() {
    assert_eq!(pack_bytes(&[]).unwrap(), &[]);
    assert_eq!(pack_bytes(&[0, 0]).unwrap(), &[0]);
    assert_eq!(pack_bytes(&[0, 1]).unwrap(), &[1]);
    assert_eq!(pack_bytes(&[1, 0]).unwrap(), &[256]);
    assert_eq!(pack_bytes(&[1, 1]).unwrap(), &[257]);
    assert_eq!(pack_bytes(&[0, 1, 0, 2]).unwrap(), &[1, 2]);
    assert_eq!(pack_bytes(&[1, 1, 1, 2]).unwrap(), &[257, 258]);
    assert!(pack_bytes(&[1]).is_err());
    assert!(pack_bytes(&[1, 2, 3]).is_err());
}
