use vm_memory::{Bytes, GuestAddress, GuestMemory};
use zerocopy::little_endian::{U16, U32, U64};
use zerocopy::AsBytes;

use crate::{checksum, GenericAddressStructure, Result, Sdt, SdtHeader};

#[cfg(target_arch = "x86_64")]
pub const IAPC_BOOT_ARG_FLAGS_VGA_NOT_PRESENT: u16 = 2;
#[cfg(target_arch = "x86_64")]
pub const IAPC_BOOT_ARG_FLAGS_MSI_NOT_PRESENT: u16 = 3;
#[cfg(target_arch = "x86_64")]
pub const IAPC_BOOT_ARG_FLAGS_PCI_ASPM: u16 = 4;

/// ACPI Flags

/// Flag for the Power Button functionality. Reading from the specification here:
/// https://uefi.org/specs/ACPI/6.5/05_ACPI_Software_Programming_Model.html#fixed-acpi-description-table-fixed-feature-flags
/// "If the system does not have a power button, this value would be “1” and no power button device
/// would be present"
pub const FADT_F_PWR_BUTTON: u8 = 4;
/// Flag for the Sleep Button Functionality.
pub const FADT_F_SLP_BUTTON: u8 = 5;
/// Flag for Hardware Reduced API
pub const FADT_F_HW_REDUCED_ACPI: u8 = 20;

#[repr(packed)]
#[derive(Debug, Copy, Clone, Default, AsBytes)]
pub struct Fadt {
    pub header: SdtHeader,
    pub firmware_control: U32,
    pub dsdt: U32,
    pub reserved_1: u8,
    pub preferred_pm_profile: u8,
    pub sci_int: U16,
    pub smi_cmd: U32,
    pub acpi_enable: u8,
    pub acpi_disable: u8,
    pub s4bios_req: u8,
    pub pstate_cnt: u8,
    pub pm1a_evt_blk: U32,
    pub pm1b_evt_blk: U32,
    pub pm1a_cnt_blk: U32,
    pub pm1b_cnt_blk: U32,
    pub pm2_cnt_blk: U32,
    pub pm_tmr_blk: U32,
    pub gpe0_blk: U32,
    pub gpe1_blk: U32,
    pub pm1_evt_len: u8,
    pub pm1_cnt_len: u8,
    pub pm2_cnt_len: u8,
    pub pm_tmr_len: u8,
    pub gpe0_blk_len: u8,
    pub gpe1_blk_len: u8,
    pub gpe1_base: u8,
    pub cst_cnt: u8,
    pub p_lvl2_lat: U16,
    pub p_lvl3_lat: U16,
    pub flush_size: U16,
    pub flush_stride: U16,
    pub duty_offset: u8,
    pub duty_width: u8,
    pub day_alrm: u8,
    pub mon_alrm: u8,
    pub century: u8,
    pub iapc_boot_arch: U16,
    pub reserved_2: u8,
    pub flags: U32,
    pub reset_reg: GenericAddressStructure,
    pub reset_value: u8,
    pub arm_boot_arch: U16,
    pub fadt_minor_version: u8,
    pub x_firmware_ctrl: U64,
    pub x_dsdt: U64,
    pub x_pm1a_evt_blk: GenericAddressStructure,
    pub x_pm1b_evt_blk: GenericAddressStructure,
    pub x_pm1a_cnt_blk: GenericAddressStructure,
    pub x_pm1b_cnt_blk: GenericAddressStructure,
    pub x_pm2_cnt_blk: GenericAddressStructure,
    pub x_pm_tmr_blk: GenericAddressStructure,
    pub x_gpe0_blk: GenericAddressStructure,
    pub x_gpe1_blk: GenericAddressStructure,
    pub sleep_control_reg: GenericAddressStructure,
    pub sleep_status_reg: GenericAddressStructure,
    pub hypervisor_vendor_id: [u8; 8],
}

impl Fadt {
    pub fn new(
        oem_id: [u8; 6],
        oem_table_id: [u8; 8],
        oem_revision: u32,
        x_dsdt_addr: u64,
        sci_int: u16,
        x_pm1a_evt_blk: GenericAddressStructure,
        x_pm1a_cnt_blk: GenericAddressStructure,
        hypervisor_vendor_id: [u8; 8],
    ) -> Self {
        assert_eq!(std::mem::size_of::<Self>(), 276);
        let header = SdtHeader::new(
            *b"FACP",
            // It's fine to unwrap here, we know that the size of the Fadt structure fits in 32
            // bits.
            std::mem::size_of::<Self>().try_into().unwrap(),
            6, // revision 6
            oem_id,
            oem_table_id,
            oem_revision,
        );

        Fadt {
            header,
            sci_int: U16::new(sci_int),
            fadt_minor_version: 5,
            x_dsdt: U64::new(x_dsdt_addr),
            hypervisor_vendor_id,
            x_pm1a_evt_blk,
            x_pm1a_cnt_blk,
            pm1_evt_len: x_pm1a_evt_blk.register_bit_width / 8,
            pm1_cnt_len: x_pm1a_cnt_blk.register_bit_width / 8,
            ..Default::default()
        }

        // fadt.pm1_evt_len = ACPI_PM1_EVT_LEN;
        // fadt.pm1_cnt_len = ACPI_PM1_CNT_LEN;
        // fadt.fadt_minor_version = FADT_MINOR_VERSION;
        // Disable FACP table
        // fadt.flags = U32::new(1) << F_HARDWARE_REDUCED_ACPI;
        // fadt.x_dsdt = x_dsdt_addr.into();
        // Disable probing for VGA, enabling MSI and PCI ASPM Controls,
        // maybe we can speed-up a bit booting
        // fadt.iapc_boot_arch = U16::new(1) << IAPC_BOOT_ARG_FLAGS_VGA_NOT_PRESENT
        // | U16::new(1) << IAPC_BOOT_ARG_FLAGS_MSI_NOT_PRESENT
        // | U16::new(1) << IAPC_BOOT_ARG_FLAGS_PCI_ASPM;
        //
        // let mut acpi_register_offset = ACPI_REGISTERS_BASE_ADDRESS;
        // fadt.x_pm1a_evt_blk =
        // GenericAddressStructure::io_port_address::<u32>(acpi_register_offset);
        //
        // acpi_register_offset += ACPI_PM1_EVT_LEN as u16;
        // fadt.x_pm1a_cnt_blk =
        // GenericAddressStructure::io_port_address::<u16>(acpi_register_offset);
    }

    pub fn set_flags(&mut self, flags: u32) {
        self.flags = U32::new(flags);
    }

    #[cfg(target_arch = "x86_64")]
    pub fn setup_iapc_flags(&mut self, flags: u16) {
        self.iapc_boot_arch = U16::new(flags);
    }
}

impl Sdt for Fadt {
    fn len(&self) -> usize {
        self.header.length.get().try_into().unwrap()
    }

    fn write_to_guest<M: GuestMemory>(&mut self, mem: &M, address: GuestAddress) -> Result<()> {
        self.header.set_checksum(checksum(&[self.as_bytes()]));
        assert_eq!(checksum(&[self.as_bytes()]), 0);
        mem.write_slice(self.as_bytes(), address)?;
        Ok(())
    }
}
