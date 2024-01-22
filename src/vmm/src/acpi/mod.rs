use std::rc::Rc;

use crate::{
    device_manager::{legacy::PortIODeviceManager, mmio::MMIODeviceManager},
    vstate::memory::{GuestAddress, GuestMemoryMmap},
    Vcpu,
};
use acpi_tables::{
    aml,
    fadt::{FADT_F_HW_REDUCED_ACPI, FADT_F_PWR_BUTTON, FADT_F_SLP_BUTTON},
    Aml,
};
use acpi_tables::{AddressSpace, Dsdt, Fadt, GenericAddressStructure, Madt, Rsdp, Sdt, Xsdt};
use log::debug;
use vm_allocator::AllocPolicy;

use crate::{arch, device_manager::resources::ResourceAllocator};

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
use x86_64::*;

// Our (Original Equipment Manufacturer" (OEM) name. OEM is how ACPI names the manufacturer of the
// hardware that is exposed to the OS, through ACPI tables. The OEM name is passed in every ACPI
// table, to let the OS know that we are the owner of the table.
// More information here:
const OEM_ID: [u8; 6] = *b"FIRECK";

// In reality the OEM revision is per table and it defines the revision of the OEM's implementation
// of the particular ACPI table. For our purpose, we can set it to a fixed value for all the tables
const OEM_REVISION: u32 = 0;

// This is needed for an entry in the FADT table. Populating this entry in FADT is a way to let the guest
// know that it runs within a Firecracker microVM.
const HYPERVISOR_VENDOR_ID: [u8; 8] = *b"FIRECKVM";

#[derive(Debug, thiserror::Error, displaydoc::Display)]
/// Error type for ACPI related operations
pub enum AcpiManagerError {
    /// Could not allocate resources: {0}
    VmAllocator(#[from] vm_allocator::Error),
    /// ACPI tables error: {0}
    AcpiTables(#[from] acpi_tables::AcpiError),
}

#[derive(Debug)]
pub(crate) struct AcpiManager {
    resource_allocator: Rc<ResourceAllocator>,
    rsdp_addr: GuestAddress,
}

impl AcpiManager {
    pub(crate) fn new(resource_allocator: Rc<ResourceAllocator>) -> Result<Self, AcpiManagerError> {
        Ok(Self {
            resource_allocator,
            rsdp_addr: GuestAddress(0x000e_0000),
        })
    }

    fn write_acpi_table<S>(
        &mut self,
        mem: &GuestMemoryMmap,
        table: &mut S,
    ) -> Result<u64, AcpiManagerError>
    where
        S: Sdt,
    {
        let addr = self.resource_allocator.allocate_acpi_memory(
            table.len().try_into().unwrap(),
            64,
            AllocPolicy::FirstMatch,
        )?;

        table.write_to_guest(mem, GuestAddress(addr))?;

        Ok(addr)
    }

    fn build_dsdt(
        &mut self,
        mem: &GuestMemoryMmap,
        vcpus: &[Vcpu],
        mmio: &MMIODeviceManager,
        pio: &PortIODeviceManager,
    ) -> Result<u64, AcpiManagerError> {
        debug!("acpi: building DSDT table");
        let mut dsdt_data = Vec::new();

        // CPU-related Aml data
        let hid = aml::Name::new("_HID".into(), &"ACPI0010");
        let uid = aml::Name::new("_CID".into(), &aml::EisaName::new("PNP0A05"));
        let cpu_methods = aml::Method::new("CSCN".into(), 0, true, vec![]);
        let mut cpu_inner_data: Vec<&dyn Aml> = vec![&hid, &uid, &cpu_methods];
        for vcpu in vcpus {
            cpu_inner_data.push(vcpu);
        }
        aml::Device::new("_SB_.CPUS".into(), cpu_inner_data).append_aml_bytes(&mut dsdt_data);

        // Virtio-devices DSDT data
        mmio.append_aml_bytes(&mut dsdt_data);

        // Legacy-IO devices DSDT data
        pio.append_aml_bytes(&mut dsdt_data);

        let mut dsdt = Dsdt::new(OEM_ID, *b"FCVMDSDT", OEM_REVISION, dsdt_data);
        self.write_acpi_table(mem, &mut dsdt)
    }

    fn build_fadt(
        &mut self,
        mem: &GuestMemoryMmap,
        dsdt_addr: u64,
    ) -> Result<u64, AcpiManagerError> {
        debug!("acpi: building FADT table");
        // TODO: properly maintain the addresses for these two.
        let x_pm1a_evt_blk =
            GenericAddressStructure::new(AddressSpace::SystemIO as u8, 32, 0, 4, 0x500);
        let x_pm1a_cnt_blk =
            GenericAddressStructure::new(AddressSpace::SystemIO as u8, 16, 0, 2, 0x504);
        let mut fadt = Fadt::new(
            OEM_ID,
            *b"FCVMFADT",
            OEM_REVISION,
            dsdt_addr,
            9, //TODO: properly maintain this
            x_pm1a_evt_blk,
            x_pm1a_cnt_blk,
            HYPERVISOR_VENDOR_ID,
        );
        fadt.set_flags(
            1 << FADT_F_HW_REDUCED_ACPI | 1 << FADT_F_PWR_BUTTON | 1 << FADT_F_SLP_BUTTON,
        );
        setup_arch_fadt(&mut fadt);
        self.write_acpi_table(mem, &mut fadt)
    }

    fn build_madt(
        &mut self,
        mem: &GuestMemoryMmap,
        vcpus: &[Vcpu],
    ) -> Result<u64, AcpiManagerError> {
        debug!("acpi: building MADT table");
        let mut madt = Madt::new(OEM_ID, *b"FCVMMADT", OEM_REVISION, arch::APIC_ADDR);
        setup_interrupt_controllers(&mut madt, vcpus.len().try_into().unwrap());
        self.write_acpi_table(mem, &mut madt)
    }

    pub(crate) fn create_acpi_tables(
        &mut self,
        mem: &GuestMemoryMmap,
        vcpus: &[Vcpu],
        mmio: &MMIODeviceManager,
        pio: &PortIODeviceManager,
    ) -> Result<(), AcpiManagerError> {
        let dsdt_addr = self.build_dsdt(mem, vcpus, mmio, pio)?;
        let fadt_addr = self.build_fadt(mem, dsdt_addr)?;
        let madt_addr = self.build_madt(mem, vcpus)?;

        let mut xsdt = Xsdt::new(
            OEM_ID,
            *b"FCMVXSDT",
            OEM_REVISION,
            vec![fadt_addr, madt_addr],
        );
        let xsdt_addr = self.write_acpi_table(mem, &mut xsdt)?;

        let mut rsdp = Rsdp::new(OEM_ID, xsdt_addr);
        rsdp.write_to_guest(mem, self.rsdp_addr)?;

        Ok(())
    }
}
