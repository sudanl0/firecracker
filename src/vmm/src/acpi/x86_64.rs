use acpi_tables::fadt::{
    IAPC_BOOT_ARG_FLAGS_MSI_NOT_PRESENT, IAPC_BOOT_ARG_FLAGS_PCI_ASPM,
    IAPC_BOOT_ARG_FLAGS_VGA_NOT_PRESENT,
};
use acpi_tables::{Fadt, Madt};

use crate::arch::IOAPIC_ADDR;

pub(crate) fn setup_interrupt_controllers(madt: &mut Madt, nr_cpus: u8) {
    madt.setup_ioapic(IOAPIC_ADDR);
    madt.setup_local_apic(nr_cpus);
}

pub(crate) fn setup_arch_fadt(fadt: &mut Fadt) {
    fadt.setup_iapc_flags(
        1 << IAPC_BOOT_ARG_FLAGS_VGA_NOT_PRESENT
            | 1 << IAPC_BOOT_ARG_FLAGS_PCI_ASPM
            | 1 << IAPC_BOOT_ARG_FLAGS_MSI_NOT_PRESENT,
    );
}
