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
#[derive(Clone, Copy, Debug, Default, AsBytes)]
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
