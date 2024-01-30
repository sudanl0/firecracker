use acpi_tables::{Fadt, Madt};

#[allow(unused_variables)]
pub(crate) fn setup_interrupt_controllers(madt: &mut Madt, nr_cpus: u8) {}

#[allow(unused_variables)]
pub(crate) fn setup_arch_fadt(fadt: &mut Fadt) {}
