use std::mem::size_of;

use vm_memory::{Address, Bytes, GuestAddress, GuestMemory};
use zerocopy::AsBytes;

use crate::{checksum, AcpiError, Result, Sdt, SdtHeader};

#[derive(Clone, Default, Debug)]
pub struct Xsdt {
    header: SdtHeader,
    tables: Vec<u8>,
}

impl Xsdt {
    pub fn new(
        oem_id: [u8; 6],
        oem_table_id: [u8; 8],
        oem_revision: u32,
        tables: Vec<u64>,
    ) -> Self {
        let mut tables_bytes = Vec::with_capacity(8 * tables.len());
        for addr in tables {
            tables_bytes.extend(&addr.to_le_bytes());
        }

        let header = SdtHeader::new(
            *b"XSDT",
            (std::mem::size_of::<SdtHeader>() + tables_bytes.len()) as u32,
            1,
            oem_id,
            oem_table_id,
            oem_revision,
        );

        let mut xsdt = Xsdt {
            header,

            tables: tables_bytes,
        };

        xsdt.header.set_checksum(checksum(&[
            xsdt.header.as_bytes(),
            (xsdt.tables.as_slice()),
        ]));

        xsdt
    }
}

impl Sdt for Xsdt {
    fn len(&self) -> usize {
        std::mem::size_of::<SdtHeader>() + self.tables.len()
    }

    fn write_to_guest<M: GuestMemory>(&mut self, mem: &M, address: GuestAddress) -> Result<()> {
        mem.write_slice(self.header.as_bytes(), address)?;
        let address = address
            .checked_add(size_of::<SdtHeader>() as u64)
            .ok_or(AcpiError::InvalidGuestAddress)?;
        mem.write_slice(self.tables.as_slice(), address)?;
        Ok(())
    }
}
