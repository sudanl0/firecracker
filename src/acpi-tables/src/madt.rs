use std::mem::size_of;

use vm_memory::{Address, Bytes, GuestAddress, GuestMemory};
use zerocopy::little_endian::U32;
use zerocopy::AsBytes;

use crate::{checksum, AcpiError, Result, Sdt, SdtHeader};

const MADT_CPU_ENABLE_FLAG: u32 = 0;

#[repr(packed)]
#[derive(Copy, Clone, Debug, Default, AsBytes)]
pub struct LocalAPIC {
    _type: u8,
    _length: u8,
    _processor_uid: u8,
    _apic_id: u8,
    _flags: U32,
}

impl LocalAPIC {
    pub fn new(cpu_id: u8) -> Self {
        Self {
            _type: 0,
            _length: 8,
            _processor_uid: cpu_id,
            _apic_id: cpu_id,
            _flags: U32::new(1u32 << MADT_CPU_ENABLE_FLAG),
        }
    }
}

#[repr(packed)]
#[derive(Copy, Clone, Debug, Default, AsBytes)]
pub struct IoAPIC {
    _type: u8,
    _length: u8,
    _ioapic_id: u8,
    _reserved: u8,
    _apic_address: U32,
    _gsi_base: U32,
}

impl IoAPIC {
    pub fn new(ioapic_id: u8, apic_address: u32) -> Self {
        IoAPIC {
            _type: 1,
            _length: 12,
            _ioapic_id: ioapic_id,
            _reserved: 0,
            _apic_address: U32::new(apic_address),
            _gsi_base: U32::ZERO,
        }
    }
}

#[derive(Debug)]
pub struct Madt {
    header: SdtHeader,
    base_address: U32,
    flags: U32,
    interrupt_controllers: Vec<u8>,
}

impl Madt {
    pub fn new(
        oem_id: [u8; 6],
        oem_table_id: [u8; 8],
        oem_revision: u32,
        base_address: u32,
    ) -> Self {
        // It is ok to unwrap the conversion of the size of `SdtHeader` to u32, because we know the
        // length of the header
        let length = 8 + size_of::<SdtHeader>();
        let header = SdtHeader::new(
            *b"APIC",
            length.try_into().unwrap(),
            6,
            oem_id,
            oem_table_id,
            oem_revision,
        );

        Madt {
            header,
            base_address: U32::new(base_address),
            flags: U32::ZERO,
            interrupt_controllers: Vec::new(),
        }
    }

    fn add_interrupt_controller(&mut self, ic: &[u8]) {
        self.interrupt_controllers.extend(ic);
        self.header.length += U32::new(ic.len().try_into().unwrap());
    }

    #[cfg(target_arch = "x86_64")]
    pub fn setup_ioapic(&mut self, ioapic_address: u32) {
        self.add_interrupt_controller(IoAPIC::new(0, ioapic_address).as_bytes());
    }

    #[cfg(target_arch = "x86_64")]
    pub fn setup_local_apic(&mut self, nr_cpus: u8) {
        for cpu_id in 0..nr_cpus {
            let lapic = LocalAPIC::new(cpu_id);
            self.add_interrupt_controller(lapic.as_bytes());
        }
    }
}

impl Sdt for Madt {
    fn len(&self) -> usize {
        self.header.length.get().try_into().unwrap()
    }

    fn write_to_guest<M: GuestMemory>(&mut self, mem: &M, address: GuestAddress) -> Result<()> {
        // Set the correct checksum in the header before writing the table in guest memory
        self.header.set_checksum(checksum(&[
            self.header.as_bytes(),
            self.base_address.as_bytes(),
            self.flags.as_bytes(),
            self.interrupt_controllers.as_bytes(),
        ]));
        mem.write_slice(self.header.as_bytes(), address)?;
        let address = address
            .checked_add(size_of::<SdtHeader>() as u64)
            .ok_or(AcpiError::InvalidGuestAddress)?;
        mem.write_slice(self.base_address.as_bytes(), address)?;
        let address = address
            .checked_add(size_of::<u32>() as u64)
            .ok_or(AcpiError::InvalidGuestAddress)?;
        mem.write_slice(self.flags.as_bytes(), address)?;
        let address = address
            .checked_add(size_of::<u32>() as u64)
            .ok_or(AcpiError::InvalidGuestAddress)?;
        mem.write_slice(self.interrupt_controllers.as_bytes(), address)?;

        Ok(())
    }
}
