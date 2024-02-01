use std::rc::Rc;

use acpi_tables::fadt::{FADT_F_HW_REDUCED_ACPI, FADT_F_PWR_BUTTON, FADT_F_SLP_BUTTON};
use acpi_tables::{
    aml, AddressSpace, Aml, Dsdt, Fadt, GenericAddressStructure, Madt, Rsdp, Sdt, Xsdt,
};
#[cfg(target_arch = "aarch64")]
use acpi_tables::{ Gtdt, Pptt,};
#[cfg(target_arch = "aarch64")]
use linux_loader::cmdline::Cmdline as LoaderKernelCmdline;
use log::debug;
use vm_allocator::AllocPolicy;

use crate::arch;
use crate::device_manager::resources::ResourceAllocator;
#[cfg(target_arch = "aarch64")]
use crate::{
    device_manager::mmio::MMIODeviceManager,
    vstate::memory::{GuestAddress, GuestMemoryMmap},
    Vcpu,
};
#[cfg(target_arch = "x86_64")]
use crate::{
    device_manager::{legacy::PortIODeviceManager, mmio::MMIODeviceManager},
    vstate::memory::{GuestAddress, GuestMemoryMmap},
    Vcpu,
};

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "aarch64")]
mod aarch64;

#[cfg(target_arch = "aarch64")]
use aarch64::*;
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

// This is needed for an entry in the FADT table. Populating this entry in FADT is a way to let the
// guest know that it runs within a Firecracker microVM.
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
            rsdp_addr: GuestAddress(arch::ACPI_RSDP),
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
        #[cfg(target_arch = "x86_64")] pio: &PortIODeviceManager,
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

        // Virtio-devices DSDT data (also adds serial for aarch64)
        mmio.append_aml_bytes(&mut dsdt_data);

        #[cfg(target_arch = "x86_64")]
        // Legacy-IO devices DSDT data
        pio.append_aml_bytes(&mut dsdt_data);

        // Can add cpu hotplug and memory hotplug in the future
        let mut dsdt = Dsdt::new(OEM_ID, *b"FCVMDSDT", OEM_REVISION, dsdt_data);
        debug!("{:#x?}", dsdt);
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
            9, // TODO: properly maintain this
            x_pm1a_evt_blk,
            x_pm1a_cnt_blk,
            HYPERVISOR_VENDOR_ID,
        );
        fadt.set_flags(
            1 << FADT_F_HW_REDUCED_ACPI | 1 << FADT_F_PWR_BUTTON | 1 << FADT_F_SLP_BUTTON,
        );
        setup_arch_fadt(&mut fadt);
        debug!("{:#x?}", fadt);
        self.write_acpi_table(mem, &mut fadt)
    }

    fn build_madt(
        &mut self,
        mem: &GuestMemoryMmap,
        vcpus: &[Vcpu],
        #[cfg(target_arch = "aarch64")] gic: &arch::aarch64::gic::GICDevice,
    ) -> Result<u64, AcpiManagerError> {
        debug!("acpi: building MADT table");
        let mut madt = Madt::new(OEM_ID, *b"FCVMMADT", OEM_REVISION, arch::APIC_ADDR);
        #[cfg(target_arch = "x86_64")]
        setup_interrupt_controllers(&mut madt, vcpus.len().try_into().unwrap());

        // pass vcpus to extract nr_cpus and mpidr
        #[cfg(target_arch = "aarch64")]
        setup_interrupt_controllers(&mut madt, vcpus, gic);
        debug!("{:#x?}", madt);

        self.write_acpi_table(mem, &mut madt)
    }

    #[cfg(target_arch = "aarch64")]
    fn build_pptt(
        &mut self,
        mem: &GuestMemoryMmap,
        vcpus: &[Vcpu],
    ) -> Result<u64, AcpiManagerError> {
        let mut pptt = Pptt::new(
            OEM_ID,
            *b"FCVMPPTT",
            OEM_REVISION,
            vcpus.len().try_into().unwrap(),
        );
        debug!("{:#x?}", pptt);
        self.write_acpi_table(mem, &mut pptt)
    }

    #[cfg(target_arch = "aarch64")]
    fn build_gtdt(&mut self, mem: &GuestMemoryMmap) -> Result<u64, AcpiManagerError> {
        let mut gtdt = Gtdt::new(OEM_ID, *b"FCVMGTDT", OEM_REVISION);
        debug!("{:#x?}", gtdt);
        self.write_acpi_table(mem, &mut gtdt)
    }

    pub(crate) fn create_acpi_tables(
        &mut self,
        mem: &GuestMemoryMmap,
        vcpus: &[Vcpu],
        mmio: &MMIODeviceManager,
        #[cfg(target_arch = "x86_64")] pio: &PortIODeviceManager,
        #[cfg(target_arch = "aarch64")] gic: &arch::aarch64::gic::GICDevice,
        #[cfg(target_arch = "aarch64")] cmdline: &mut LoaderKernelCmdline,
    ) -> Result<(), AcpiManagerError> {
        #[cfg(target_arch = "x86_64")]
        let dsdt_addr = self.build_dsdt(mem, vcpus, mmio, pio)?;
        #[cfg(target_arch = "aarch64")]
        let dsdt_addr = self.build_dsdt(mem, vcpus, mmio)?;
        let fadt_addr = self.build_fadt(mem, dsdt_addr)?;
        let madt_addr = self.build_madt(
            mem,
            vcpus,
            #[cfg(target_arch = "aarch64")]
            gic,
        )?;

        #[cfg(target_arch = "aarch64")]
        let pptt_addr = self.build_pptt(mem, vcpus)?;
        #[cfg(target_arch = "aarch64")]
        let gtdt_addr = self.build_gtdt(mem)?;

        // SPCR is useful when earlycon= is used with no options
        // When used with no options, the early console is
        // 	determined by stdout-path property in device tree's
        // 	chosen node or the ACPI SPCR table if supported by
        // 	the platform.

        let mut xsdt = Xsdt::new(
            OEM_ID,
            *b"FCMVXSDT",
            OEM_REVISION,
            vec![fadt_addr, madt_addr, #[cfg(target_arch = "aarch64")] pptt_addr, #[cfg(target_arch = "aarch64")] gtdt_addr],
        );
        debug!("{:#x?}", xsdt);
        let xsdt_addr = self.write_acpi_table(mem, &mut xsdt)?;

        let mut rsdp = Rsdp::new(OEM_ID, xsdt_addr);
        debug!("{:#x?}", rsdp);
        debug!(
            "\nfadt_addr:{:#x?},\n madt_addr:{:#x?},\n \
             xsdt_addr:{:#x?},\n self.rsdp_addr:{:#x?}\n",
            fadt_addr, madt_addr, xsdt_addr, self.rsdp_addr
        );
        #[cfg(target_arch = "aarch64")]
        debug!("pptt_addr:{:#x?},\n gtdt_addr:{:#x?}\n", pptt_addr, gtdt_addr);
        rsdp.write_to_guest(mem, self.rsdp_addr)?;
        #[cfg(target_arch = "aarch64")]
        let acpi_cmdline = format!("acpi=force acpi_rsdp={:#x?}", self.rsdp_addr.0);
        #[cfg(target_arch = "aarch64")]
        cmdline
            .insert_str(acpi_cmdline)
            .expect("inserting acpi rsdp to cmdline failed");

        Ok(())
    }
}
