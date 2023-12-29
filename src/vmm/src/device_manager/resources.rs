// Copyright 2023 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::cell::RefCell;

pub use vm_allocator::AllocPolicy;
use vm_allocator::{AddressAllocator, IdAllocator};

use crate::arch;

/// A resource manager for (de)allocating interrupt lines (GSIs) and guest memory
///
/// At the moment, we support:
///
/// * GSIs for legacy x86_64 devices
/// * GSIs for MMIO devicecs
/// * Memory allocations in the MMIO address space
#[derive(Debug)]
pub struct ResourceAllocator {
    // Allocator for device interrupt lines
    gsi_allocator: RefCell<IdAllocator>,
    // Allocator for memory in the MMIO address space
    mmio_memory: RefCell<AddressAllocator>,
}

impl ResourceAllocator {
    /// Create a new resource allocator for Firecracker devices
    pub fn new() -> Result<Self, vm_allocator::Error> {
        Ok(Self {
            gsi_allocator: RefCell::new(IdAllocator::new(arch::IRQ_BASE, arch::IRQ_MAX)?),
            mmio_memory: RefCell::new(AddressAllocator::new(
                arch::MMIO_MEM_START,
                arch::MMIO_MEM_SIZE,
            )?),
        })
    }

    /// Allocate a number of GSIs
    pub fn allocate_gsi(&self, gsi_count: u32) -> Result<Vec<u32>, vm_allocator::Error> {
        let mut gsis = Vec::with_capacity(gsi_count as usize);

        for _ in 0..gsi_count {
            let mut allocator = self.gsi_allocator.borrow_mut();
            match allocator.allocate_id() {
                Ok(gsi) => gsis.push(gsi),
                Err(err) => {
                    // It is ok to unwrap here, we just allocated the GSI
                    gsis.into_iter().for_each(|gsi| {
                        allocator.free_id(gsi).unwrap();
                    });
                    return Err(err);
                }
            }
        }

        Ok(gsis)
    }

    /// Allocate a memory range in MMIO address space
    ///
    /// If it succeeds, it returns the first address of the allocated range
    ///
    /// # Arguments
    ///
    /// * `size` - The size in bytes of the memory to allocate
    /// * `alignment` - The alignment of the address of the first byte
    /// * `policy` - A [`vm_allocator::AllocPolicy`] variant for determining the allocation policy
    pub fn allocate_mmio_memory(
        &self,
        size: u64,
        alignment: u64,
        policy: AllocPolicy,
    ) -> Result<u64, vm_allocator::Error> {
        Ok(self
            .mmio_memory
            .borrow_mut()
            .allocate(size, alignment, policy)?
            .start())
    }
}
