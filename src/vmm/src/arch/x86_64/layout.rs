// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

/// Magic addresses externally used to lay out x86_64 VMs.

/// Initial stack for the boot CPU.
pub const BOOT_STACK_POINTER: u64 = 0x8ff0;

/// Kernel command line start address.
pub const CMDLINE_START: u64 = 0x20000;
/// Kernel command line maximum size.
pub const CMDLINE_MAX_SIZE: usize = 2048;

/// Start of the high memory.
pub const HIMEM_START: u64 = 0x0010_0000; // 1 MB.

// Typically, on x86 systems 24 IRQs are used (0-23).
/// First usable IRQ ID for virtio device interrupts on x86_64.
pub const IRQ_BASE: u32 = 5;
/// Last usable IRQ ID for virtio device interrupts on x86_64.
pub const IRQ_MAX: u32 = 23;

/// Address for the TSS setup.
pub const KVM_TSS_ADDRESS: u64 = 0xfffb_d000;

/// The 'zero page', a.k.a linux kernel bootparams.
pub const ZERO_PAGE_START: u64 = 0x7000;

/// APIC address
pub const APIC_ADDR: u32 = 0xfee0_0000;

/// IOAPIC address
pub const IOAPIC_ADDR: u32 = 0xfec0_0000;

/// Start of memory region we will use for ACPI data. We are putting them
/// at the beginning of High Memory.
pub const ACPI_MEM_START: u64 = HIMEM_START;

/// Size of memory region for ACPI data (1KB of memory at the moment).
pub const ACPI_MEM_SIZE: u64 = 4096;
