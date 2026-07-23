use crate::{Address, Error, Process, ReadOnly, Result};

const DOS_HEADER_SIZE: usize = 64;
const COFF_HEADER_SIZE: usize = 24;
const SECTION_HEADER_SIZE: usize = 40;
const PE32_MAGIC: u16 = 0x10b;
const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
const IMAGE_SCN_MEM_WRITE: u32 = 0x8000_0000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeIdentity {
    pub machine: u16,
    pub timestamp: u32,
    pub mapped_image_base: Address,
    pub preferred_image_base: u32,
    pub image_size: u32,
    pub checksum: u32,
    pub relocation_address: Address,
    pub relocation_size: u32,
    pub sections: Vec<PeSection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeSection {
    pub name: String,
    pub address: Address,
    pub virtual_size: u32,
    pub mapped_size: u32,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

impl PeSection {
    pub fn immutable(&self) -> bool {
        self.readable && !self.writable
    }
}

fn invalid_data(address: Address, message: &'static str) -> Error {
    Error::ReadFailed {
        address: address.value(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, message),
    }
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

pub fn read_mapped_pe(process: &Process<ReadOnly>, base: Address) -> Result<PeIdentity> {
    let dos = process.read_bytes_at(base, DOS_HEADER_SIZE)?;
    if dos.get(0..2) != Some(b"MZ") {
        return Err(invalid_data(base, "missing MZ signature"));
    }
    let pe_offset =
        u64::from(u32_at(&dos, 0x3c).ok_or_else(|| invalid_data(base, "missing PE offset"))?);
    let pe_address = base.checked_add(pe_offset)?;
    let coff = process.read_bytes_at(pe_address, COFF_HEADER_SIZE)?;
    if coff.get(0..4) != Some(b"PE\0\0") {
        return Err(invalid_data(pe_address, "missing PE signature"));
    }
    let machine = u16_at(&coff, 4).ok_or_else(|| invalid_data(pe_address, "missing machine"))?;
    let section_count =
        u16_at(&coff, 6).ok_or_else(|| invalid_data(pe_address, "missing section count"))?;
    let timestamp =
        u32_at(&coff, 8).ok_or_else(|| invalid_data(pe_address, "missing timestamp"))?;
    let optional_size = usize::from(
        u16_at(&coff, 20).ok_or_else(|| invalid_data(pe_address, "missing optional size"))?,
    );
    if optional_size < 68 {
        return Err(invalid_data(pe_address, "optional header is too small"));
    }
    let optional_address = pe_address.checked_add(COFF_HEADER_SIZE as u64)?;
    let optional = process.read_bytes_at(optional_address, optional_size)?;
    if u16_at(&optional, 0) != Some(PE32_MAGIC) {
        return Err(invalid_data(optional_address, "mapped module is not PE32"));
    }
    let preferred_image_base = u32_at(&optional, 28)
        .ok_or_else(|| invalid_data(optional_address, "missing image base"))?;
    let image_size = u32_at(&optional, 56)
        .ok_or_else(|| invalid_data(optional_address, "missing image size"))?;
    let checksum =
        u32_at(&optional, 64).ok_or_else(|| invalid_data(optional_address, "missing checksum"))?;
    if optional_size < 144 {
        return Err(invalid_data(
            optional_address,
            "optional header has no relocation directory",
        ));
    }
    let relocation_rva = u32_at(&optional, 136)
        .ok_or_else(|| invalid_data(optional_address, "missing relocation RVA"))?;
    let relocation_size = u32_at(&optional, 140)
        .ok_or_else(|| invalid_data(optional_address, "missing relocation size"))?;
    let relocation_address = base.checked_add(u64::from(relocation_rva))?;

    let section_address = optional_address.checked_add(optional_size as u64)?;
    let section_bytes = process.read_bytes_at(
        section_address,
        usize::from(section_count)
            .checked_mul(SECTION_HEADER_SIZE)
            .ok_or(Error::AddressOverflow)?,
    )?;
    let mut sections = Vec::with_capacity(usize::from(section_count));
    for header in section_bytes.chunks_exact(SECTION_HEADER_SIZE) {
        let name_end = header[..8].iter().position(|byte| *byte == 0).unwrap_or(8);
        let name = String::from_utf8_lossy(&header[..name_end]).into_owned();
        let virtual_size = u32_at(header, 8).unwrap_or(0);
        let virtual_address = u32_at(header, 12).unwrap_or(0);
        let raw_size = u32_at(header, 16).unwrap_or(0);
        let characteristics = u32_at(header, 36).unwrap_or(0);
        let mapped_size = virtual_size.max(raw_size);
        let address = base.checked_add(u64::from(virtual_address))?;
        address.validate_for(process.architecture())?;
        address
            .checked_add(u64::from(mapped_size.saturating_sub(1)))?
            .validate_for(process.architecture())?;
        sections.push(PeSection {
            name,
            address,
            virtual_size,
            mapped_size,
            readable: characteristics & IMAGE_SCN_MEM_READ != 0,
            writable: characteristics & IMAGE_SCN_MEM_WRITE != 0,
            executable: characteristics & IMAGE_SCN_MEM_EXECUTE != 0,
        });
    }

    Ok(PeIdentity {
        machine,
        timestamp,
        mapped_image_base: base,
        preferred_image_base,
        image_size,
        checksum,
        relocation_address,
        relocation_size,
        sections,
    })
}
