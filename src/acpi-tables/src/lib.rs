use std::{fmt, str};

use vm_memory::{GuestAddress, GuestMemory, GuestMemoryError};

pub mod aml;
pub mod dsdt;
pub mod fadt;
pub mod madt;
pub mod rsdp;
pub mod xsdt;

pub use aml::Aml;
pub use dsdt::Dsdt;
pub use fadt::Fadt;
pub use madt::Madt;
pub use rsdp::Rsdp;
pub use xsdt::Xsdt;
use zerocopy::little_endian::{U32, U64};
use zerocopy::AsBytes;

// This is the creator ID that we will embed in ACPI tables that are created using this crate.
const FC_ACPI_CREATOR_ID: [u8; 4] = *b"FCAT";
// This is the created ID revision that we will embed in ACPI tables that are created using this
// crate.
const FC_ACPI_CREATOR_REVISION: u32 = 0x20240119;

// Fixed HW parameters
pub const ACPI_SCI_INT: u16 = 9;
const ACPI_PM1_EVT_LEN: u8 = 4;
const ACPI_PM1_CNT_LEN: u8 = 2;
pub const ACPI_REGISTERS_BASE_ADDRESS: u16 = 0x500;
pub const ACPI_REGISTERS_LEN: u8 = ACPI_PM1_CNT_LEN + ACPI_PM1_EVT_LEN;

fn checksum(buf: &[&[u8]]) -> u8 {
    (255 - buf
        .iter()
        .flat_map(|b| b.iter())
        .fold(0u8, |acc, x| acc.wrapping_add(*x)))
    .wrapping_add(1)
}

#[derive(Debug, thiserror::Error, displaydoc::Display)]
pub enum AcpiError {
    /// Guest memory error: {0}
    GuestMemory(#[from] GuestMemoryError),
    /// Invalid guest address
    InvalidGuestAddress,
    /// Invalid register size
    InvalidRegisterSize,
}

pub type Result<T> = std::result::Result<T, AcpiError>;

/// Address spaces that ACPI understands
/// For mor info look at
/// https://uefi.org/specs/ACPI/6.5/05_ACPI_Software_Programming_Model.html#generic-address-structure-gas
#[derive(Debug)]
#[repr(u8)]
pub enum AddressSpace {
    SystemMemory = 0x00u8,
    SystemIO = 0x01u8,
    PCI = 0x02u8,
    EmbeddedController = 0x03u8,
    SMBus = 0x04u8,
    SystemCMOS = 0x05u8,
    PciBarTarget = 0x06u8,
    IPMI = 0x07u8,
    GeneralPurposeIO = 0x08u8,
    GenericSerialBus = 0x09u8,
    PCC = 0xa0u8,
    PRM = 0xb0u8,
    FunctionalFixedHw = 0x7f,
}

#[repr(packed)]
#[derive(AsBytes, Clone, Copy, Debug, Default)]
pub struct GenericAddressStructure {
    pub address_space_id: u8,
    pub register_bit_width: u8,
    pub register_bit_offset: u8,
    pub access_size: u8,
    pub address: U64,
}

impl GenericAddressStructure {
    pub fn new(
        address_space_id: u8,
        register_bit_width: u8,
        register_bit_offset: u8,
        access_size: u8,
        address: u64,
    ) -> Self {
        Self {
            address_space_id,
            register_bit_width,
            register_bit_offset,
            access_size,
            address: U64::new(address),
        }
    }

    pub fn new_address(address_space_id: u8, access_size: u8, address: u64) -> Self {
        Self::new(address_space_id, 0, 0, access_size, address)
    }

    pub fn system_io_address(access_size: u8, address: u64) -> Self {
        Self::new_address(AddressSpace::SystemIO as u8, access_size, address)
    }
}

/// Header included in all System Descriptor Tables
#[repr(packed)]
#[derive(Clone, Copy, Default, AsBytes)]
pub struct SdtHeader {
    pub signature: [u8; 4],
    pub length: U32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: U32,
    pub creator_id: [u8; 4],
    pub creator_revison: U32,
}

impl fmt::Debug for SdtHeader {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "signature : {:#?}\n",
            str::from_utf8(&self.signature).unwrap()
        )?;
        write!(f, "length : {:#?}\n", self.length)?;
        write!(f, "revision : {:#?}\n", self.revision)?;
        write!(f, "checksum : {:#?}\n", self.checksum)?;
        write!(f, "oem_id : {:#?}\n", str::from_utf8(&self.oem_id).unwrap())?;
        write!(
            f,
            "oem_table_id : {:#?}\n",
            str::from_utf8(&self.oem_table_id).unwrap()
        )?;
        write!(f, "oem_revision : {:#?}\n", self.oem_revision)?;
        write!(
            f,
            "creator_id : {:#?}\n",
            str::from_utf8(&self.creator_id).unwrap()
        )?;
        write!(f, "creator_revison : {:#?}\n", self.creator_revison)?;
        Ok(())
    }
}

impl SdtHeader {
    pub(crate) fn new(
        signature: [u8; 4],
        length: u32,
        table_revision: u8,
        oem_id: [u8; 6],
        oem_table_id: [u8; 8],
        oem_revision: u32,
    ) -> Self {
        SdtHeader {
            signature,
            length: U32::new(length),
            revision: table_revision,
            checksum: 0,
            oem_id,
            oem_table_id,
            oem_revision: U32::new(oem_revision),
            creator_id: FC_ACPI_CREATOR_ID,
            creator_revison: U32::new(FC_ACPI_CREATOR_REVISION),
        }
    }

    pub(crate) fn set_checksum(&mut self, checksum: u8) {
        self.checksum = checksum;
    }
}

/// A trait for functionality around System Descriptor Tables.
pub trait Sdt {
    /// Get the length of the table
    fn len(&self) -> usize;

    /// Return true if Sdt is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Write the table in guest memory
    fn write_to_guest<M: GuestMemory>(&mut self, mem: &M, address: GuestAddress) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::checksum;

    #[test]
    fn test_checksum() {
        assert_eq!(checksum(&[&[]]), 0u8);
        assert_eq!(checksum(&[]), 0u8);
        assert_eq!(checksum(&[&[1, 2, 3]]), 250u8);
        assert_eq!(checksum(&[&[1, 2, 3], &[]]), 250u8);
        assert_eq!(checksum(&[&[1, 2], &[3]]), 250u8);
        assert_eq!(checksum(&[&[1, 2], &[3], &[250]]), 0u8);
        assert_eq!(checksum(&[&[255]]), 1u8);
        assert_eq!(checksum(&[&[1, 2], &[3], &[250], &[255]]), 1u8);
    }
}
