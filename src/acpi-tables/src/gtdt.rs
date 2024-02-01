use std::fmt;
use std::mem::size_of;

use log::debug;
use vm_memory::{Address, Bytes, GuestAddress, GuestMemory};
use zerocopy::little_endian::{U32, U64};
use zerocopy::AsBytes;

use crate::{checksum, AcpiError, Result, Sdt, SdtHeader};

#[derive(Clone, Copy, Default)]
pub struct Gtdt {
    header: SdtHeader,
    inner: GtdtInner,
}

#[allow(dead_code)]
#[repr(packed)]
#[derive(AsBytes, Clone, Debug, Copy, Default)]
pub struct GtdtInner {
    cntcontrolbase_physical_address: U32,
    reserved: U64,
    secure_el1_timer_gsiv: U32,
    secure_el1_timer_flags: U32,
    non_secure_el1_timer_gsiv: U32,
    non_secure_el1_timer_flags: U32,
    virtual_el1_timer_gsiv: U32,
    virtual_el1_timer_flags: U32,
    el2_timer_gsiv: U32,
    el2_timer_flags: U32,
    cntreadbase_physical_address: U64,
    platform_timer_cnt: U32,
    platform_timer_flags: U32,
    virtual_el2_timer_gsiv: U32,
    virtual_el2_timer_flags: U32,
}

impl fmt::Debug for Gtdt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "header : {:#?}\n", self.header)?;
        write!(f, "inner : {:#?}\n", ())?;
        Ok(())
    }
}

impl Gtdt {
    pub fn new(oem_id: [u8; 6], oem_table_id: [u8; 8], oem_revision: u32) -> Self {
        let header = SdtHeader::new(
            *b"GTDT",
            size_of::<Gtdt>().try_into().unwrap(),
            2,
            oem_id,
            oem_table_id,
            oem_revision,
        );

        // Flag definitions Table 5.118, section 5.2.25
        // bit0 : timer_interrupt_mode : 1 indicates edge triggered 0 indicated level triggered
        // bit1 : timer_interrupt_polarity : 1 for active low, 0 for active high
        // bit2 : always_on_compatibility
        // bit3-31 : reserved
        const TIMER_INTERRUPT_MODE_BIT_POS: u32 = 0;
        const TIMER_INTERRUPT_POLARITY_BIT_POS: u32 = 1;
        const ALWAYS_ON_COMPATIBILITY_BIT_POS: u32 = 2;
        const TIMER_INTERRUPT_MODE_LEVEL_TRIGGERED: u32 = 0 << TIMER_INTERRUPT_MODE_BIT_POS;
        const TIMER_INTERRUPT_POLARITY_ACTIVE_HIGH: u32 = 0 << TIMER_INTERRUPT_POLARITY_BIT_POS;
        const ALWAYS_ON_ENABLED: u32 = 1 << ALWAYS_ON_COMPATIBILITY_BIT_POS;

        let gtdt = Gtdt {
            header,
            inner: GtdtInner {
                cntcontrolbase_physical_address: U32::new(0),
                secure_el1_timer_gsiv: U32::new(13 + 16),
                secure_el1_timer_flags: U32::new(
                    TIMER_INTERRUPT_MODE_LEVEL_TRIGGERED | TIMER_INTERRUPT_POLARITY_ACTIVE_HIGH,
                ),
                non_secure_el1_timer_gsiv: U32::new(14 + 16),
                non_secure_el1_timer_flags: U32::new(
                    TIMER_INTERRUPT_MODE_LEVEL_TRIGGERED
                        | TIMER_INTERRUPT_POLARITY_ACTIVE_HIGH
                        | ALWAYS_ON_ENABLED,
                ),
                virtual_el1_timer_gsiv: U32::new(11 + 16),
                virtual_el1_timer_flags: U32::new(
                    TIMER_INTERRUPT_MODE_LEVEL_TRIGGERED | TIMER_INTERRUPT_POLARITY_ACTIVE_HIGH,
                ),
                el2_timer_gsiv: U32::new(10 + 16),
                el2_timer_flags: U32::new(
                    TIMER_INTERRUPT_MODE_LEVEL_TRIGGERED | TIMER_INTERRUPT_POLARITY_ACTIVE_HIGH,
                ),
                ..Default::default()
            },
        };

        gtdt
    }
}

impl Sdt for Gtdt {
    fn len(&self) -> usize {
        self.header.length.get().try_into().unwrap()
    }

    fn write_to_guest<M: GuestMemory>(&mut self, mem: &M, address: GuestAddress) -> Result<()> {
        // Set the correct checksum in the header before writing the table in guest memory
        self.header
            .set_checksum(checksum(&[self.header.as_bytes(), self.inner.as_bytes()]));
        debug!(
            "{:#x?} {:#x?} {:#x?} ",
            self,
            size_of::<SdtHeader>(),
            size_of::<GtdtInner>()
        );
        mem.write_slice(self.header.as_bytes(), address)?;
        let address = address
            .checked_add(size_of::<SdtHeader>() as u64)
            .ok_or(AcpiError::InvalidGuestAddress)?;
        mem.write_slice(self.inner.as_bytes(), address)?;
        Ok(())
    }
}
