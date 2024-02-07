use std::fmt;
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

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
#[repr(packed)]
#[derive(Debug, AsBytes)]
/// See section 5.2.12.14 GIC CPU Interface (GICC) Structure in ACPI spec.
pub struct GicC {
    pub r#type: u8,
    pub length: u8,
    pub reserved0: u16,
    pub cpu_interface_number: u32,
    pub uid: u32,
    pub flags: u32,
    pub parking_version: u32,
    pub performance_interrupt: u32,
    pub parked_address: u64,
    pub base_address: u64,
    pub gicv_base_address: u64,
    pub gich_base_address: u64,
    pub vgic_interrupt: u32,
    pub gicr_base_address: u64,
    pub mpidr: u64,
    pub proc_power_effi_class: u8,
    pub reserved1: u8,
    pub spe_overflow_interrupt: u16,
}

#[cfg(target_arch = "aarch64")]
impl GicC {
    pub fn new(cpu_id: u8, mpidr: u64) -> Self {
        // /* ARMv8 MPIDR format:
        //         Bits [63:40] Must be zero
        //         Bits [39:32] Aff3 : Match Aff3 of target processor MPIDR
        //         Bits [31:24] Must be zero
        //         Bits [23:16] Aff2 : Match Aff2 of target processor MPIDR
        //         Bits [15:8] Aff1 : Match Aff1 of target processor MPIDR
        //         Bits [7:0] Aff0 : Match Aff0 of target processor MPIDR
        // */
        let mpidr_mask = 0xff_00ff_ffff;
        Self {
            r#type: 0xB, // 5.2.12.14 GIC CPU Interface (GICC) Structure
            length: 80,
            reserved0: 0,
            cpu_interface_number: cpu_id as u32,
            uid: cpu_id as u32,
            flags: 1,
            parking_version: 0,
            performance_interrupt: 0,
            parked_address: 0,
            base_address: 0,
            gicv_base_address: 0,
            gich_base_address: 0,
            vgic_interrupt: 0,
            gicr_base_address: 0,
            mpidr: mpidr & mpidr_mask,
            proc_power_effi_class: 0,
            reserved1: 0,
            spe_overflow_interrupt: 0,
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
#[repr(packed)]
#[derive(Debug, AsBytes)]
// GIC Distributor structure. See section 5.2.12.15 in ACPI spec.
pub struct GicD {
    pub r#type: u8,
    pub length: u8,
    pub reserved0: u16,
    pub gic_id: u32,
    pub base_address: u64,
    pub global_irq_base: u32,
    pub version: u8,
    pub reserved1: [u8; 3],
}

#[cfg(target_arch = "aarch64")]
impl GicD {
    pub fn new(dist_addr: u64) -> Self {
        Self {
            r#type: 0xC,
            length: 24,
            reserved0: 0,
            gic_id: 0,
            base_address: dist_addr,
            global_irq_base: 0,
            version: 3,
            reserved1: [0; 3],
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
#[repr(packed)]
#[derive(Debug, AsBytes)]
// See 5.2.12.17 GIC Redistributor (GICR) Structure in ACPI spec.
pub struct GicR {
    pub r#type: u8,
    pub length: u8,
    pub reserved: u16,
    pub base_address: u64,
    pub range_length: u32,
}

#[cfg(target_arch = "aarch64")]
impl GicR {
    pub fn new(redists_addr: u64, redists_size: u32) -> Self {
        Self {
            r#type: 0xE,
            length: 16,
            reserved: 0,
            base_address: redists_addr,
            range_length: redists_size,
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
#[repr(packed)]
#[derive(Debug, AsBytes)]
// See 5.2.12.18 GIC Interrupt Translation Service (ITS) Structure in ACPI spec.
pub struct GicIts {
    pub r#type: u8,
    pub length: u8,
    pub reserved0: u16,
    pub translation_id: u32,
    pub base_address: u64,
    pub reserved1: u32,
}

#[cfg(target_arch = "aarch64")]
impl GicIts {
    pub fn new(its_addr: u64) -> Self {
        Self {
            r#type: 0xF,
            length: 20,
            reserved0: 0,
            translation_id: 0,
            base_address: its_addr,
            reserved1: 0,
        }
    }
}

pub struct Madt {
    header: SdtHeader,
    base_address: U32,
    flags: U32,
    interrupt_controllers: Vec<u8>,
}

impl fmt::Debug for Madt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "header : {:#?}\n", self.header)?;
        write!(f, "base_address : {:#?}\n", self.base_address)?;
        write!(f, "flags : {:#?}\n", self.flags)?;
        write!(f, "interrupt_controllers : {:#?}\n", ())?;
        Ok(())
    }
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

    pub fn add_interrupt_controller(&mut self, ic: &[u8]) {
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
        mem.write_slice(self.interrupt_controllers.as_slice(), address)?;

        Ok(())
    }
}
