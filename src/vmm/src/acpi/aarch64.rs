use acpi_tables::madt::{GicC, GicD, GicIts, GicR};
use acpi_tables::{Fadt, Madt};
use zerocopy::AsBytes;

use crate::arch::aarch64::gic::GICDevice;
use crate::Vcpu;

pub(crate) fn setup_interrupt_controllers(madt: &mut Madt, vcpus: &[Vcpu], gic: &GICDevice) {
    let nr_cpus: u8 = vcpus.len().try_into().unwrap();
    let vcpu_mpidr: Vec<u64> = vcpus.iter().map(|cpu| cpu.kvm_vcpu.get_mpidr()).collect();
    // Notes:
    // Ignore Local Interrupt Controller Address at byte offset 36 of MADT table.
    for cpu_id in 0..nr_cpus {
        let gicc = GicC::new(cpu_id, vcpu_mpidr[cpu_id as usize]);
        madt.add_interrupt_controller(gicc.as_bytes());
    }

    let gicd = GicD::new(gic.device_properties()[0]);
    madt.add_interrupt_controller(gicd.as_bytes());

    let gicr = GicR::new(
        gic.device_properties()[2],
        gic.device_properties()[3] as u32,
    );
    madt.add_interrupt_controller(gicr.as_bytes());

    // Below Redistributor area is GICv3 ITS
    // kvm.h #define KVM_VGIC_V3_ITS_SIZE		(2 * SZ_64K)
    let its_adr = gic.device_properties()[2] - 0x02_0000;
    let gicits = GicIts::new(its_adr);
    madt.add_interrupt_controller(gicits.as_bytes());
}

#[allow(unused_variables)]
pub(crate) fn setup_arch_fadt(fadt: &mut Fadt) {}
