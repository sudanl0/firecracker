use std::fmt;
use std::mem::size_of;

use vm_memory::{Address, Bytes, GuestAddress, GuestMemory};
use zerocopy::little_endian::U32;
use zerocopy::AsBytes;

use crate::{checksum, AcpiError, Result, Sdt, SdtHeader};

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
#[repr(packed)]
#[derive(AsBytes)]
struct ProcessorHierarchyNode {
    pub r#type: u8,
    pub length: u8,
    pub reserved: u16,
    pub flags: u32,
    pub parent: u32,
    pub acpi_processor_id: u32,
    pub num_private_resources: u32,
}

pub struct Pptt {
    header: SdtHeader,
    proc_hierarchy_node: Vec<u8>,
}

impl fmt::Debug for Pptt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "header : {:#?}\n", self.header)?;
        Ok(())
    }
}

impl Pptt {
    pub fn new(oem_id: [u8; 6], oem_table_id: [u8; 8], oem_revision: u32, nr_cpus: u8) -> Self {
        let header = SdtHeader::new(
            *b"PPTT",
            size_of::<SdtHeader>().try_into().unwrap(),
            2,
            oem_id,
            oem_table_id,
            oem_revision,
        );
        let mut pptt = Pptt {
            header,
            proc_hierarchy_node: Vec::new(),
        };
        // Section 5.2.30 Processor Properties Topology Table (PPTT)
        let proc_hierarchy_node_offset = size_of::<SdtHeader>() as u32;

        let hierarchy_node = ProcessorHierarchyNode {
            r#type: 0,
            length: 20,
            reserved: 0,
            flags: 0x2, // (4:0 no identical implementation,
            // 3:0 not a leaf
            // 2:0 not a thread
            // 1:1 ACPI processor ID is a valid entry
            // 0:0 does not represent phys package
            parent: 0,
            acpi_processor_id: 0 as u32,
            num_private_resources: 0,
        };
        pptt.proc_hierarchy_node.extend(hierarchy_node.as_bytes());
        pptt.header.length += U32::new(hierarchy_node.as_bytes().len().try_into().unwrap());

        for cpus in 0..nr_cpus {
            let hierarchy_node = ProcessorHierarchyNode {
                r#type: 0,
                length: 20,
                reserved: 0,
                flags: 0xA, // (4:0 no identical implementation,
                // 3:1 is a leaf
                // 2:0 not a thread
                // 1:1 ACPI processor ID is a valid entry
                // 0:0 does not represent phys package
                parent: proc_hierarchy_node_offset,
                acpi_processor_id: cpus as u32,
                num_private_resources: 0,
            };
            pptt.proc_hierarchy_node.extend(hierarchy_node.as_bytes());
            pptt.header.length += U32::new(hierarchy_node.as_bytes().len().try_into().unwrap());
        }
        pptt
    }
}

impl Sdt for Pptt {
    fn len(&self) -> usize {
        self.header.length.get().try_into().unwrap()
    }

    fn write_to_guest<M: GuestMemory>(&mut self, mem: &M, address: GuestAddress) -> Result<()> {
        // debug!("{:#?}", self);
        // Set the correct checksum in the header before writing the table in guest memory
        self.header.set_checksum(checksum(&[
            self.header.as_bytes(),
            self.proc_hierarchy_node.as_bytes(),
        ]));
        mem.write_slice(self.header.as_bytes(), address)?;
        let address = address
            .checked_add(size_of::<SdtHeader>() as u64)
            .ok_or(AcpiError::InvalidGuestAddress)?;
        mem.write_slice(self.proc_hierarchy_node.as_bytes(), address)?;

        Ok(())
    }
}
