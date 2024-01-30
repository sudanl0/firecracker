use std::{fmt, str};

use vm_memory::{Bytes, GuestAddress, GuestMemory};
use zerocopy::little_endian::{U32, U64};
use zerocopy::AsBytes;

use crate::{checksum, Result, Sdt};

/// Root System Description Pointer
///
/// This is the root pointer to the ACPI hierarchy. This is what OSs
/// are looking for in the memory when initializing ACPI. It includes
/// a pointer to XSDT
#[repr(packed)]
#[derive(Clone, Copy, Default, AsBytes)]
pub struct Rsdp {
    _signature: [u8; 8],
    checksum: u8,
    _oem_id: [u8; 6],
    _revision: u8,
    _rsdt_addr: U32,
    _length: U32,
    _xsdt_addr: U64,
    extended_checksum: u8,
    _reserved: [u8; 3],
}

impl fmt::Debug for Rsdp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "_signature: {:#?}\n",
            str::from_utf8(&self._signature).unwrap()
        )?;
        write!(f, "checksum: {:#?}\n", self.checksum)?;
        write!(
            f,
            "_oem_id: {:#?}\n",
            str::from_utf8(&self._oem_id).unwrap()
        )?;
        write!(f, "_revision: {:#?}\n", self._revision)?;
        write!(f, "_rsdt_addr: {:#?}\n", self._rsdt_addr)?;
        write!(f, "_length: {:#?}\n", self._length)?;
        write!(f, "_xsdt_addr: {:#?}\n", self._xsdt_addr)?;
        write!(f, "extended_checksum: {:#?}\n", {})?;
        write!(f, "_reserved: {:#?}\n", {})?;
        Ok(())
    }
}

impl Rsdp {
    pub fn new(oem_id: [u8; 6], xsdt_addr: u64) -> Self {
        let mut rsdp = Rsdp {
            // Space in the end of string is needed!
            _signature: *b"RSD PTR ",
            checksum: 0,
            _oem_id: oem_id,
            _revision: 2,
            _rsdt_addr: U32::ZERO,
            _length: U32::new(std::mem::size_of::<Rsdp>().try_into().unwrap()),
            _xsdt_addr: U64::new(xsdt_addr),
            extended_checksum: 0,
            _reserved: [0u8; 3],
        };

        rsdp.checksum = checksum(&[&rsdp.as_bytes()[..20]]);
        rsdp.extended_checksum = checksum(&[rsdp.as_bytes()]);
        rsdp
    }
}

impl Sdt for Rsdp {
    fn len(&self) -> usize {
        self.as_bytes().len()
    }

    fn write_to_guest<M: GuestMemory>(&mut self, mem: &M, address: GuestAddress) -> Result<()> {
        mem.write_slice(self.as_bytes(), address)?;
        Ok(())
    }
}
