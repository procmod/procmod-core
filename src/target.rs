use crate::{Error, Result};

/// Architecture of the attached process, independent of the inspector architecture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Architecture {
    X86,
    X86_64,
    Arm64,
    Unknown(u16),
}

impl Architecture {
    pub const fn pointer_width(self) -> Option<PointerWidth> {
        match self {
            Self::X86 => Some(PointerWidth::Bits32),
            Self::X86_64 | Self::Arm64 => Some(PointerWidth::Bits64),
            Self::Unknown(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerWidth {
    Bits32,
    Bits64,
}

/// An address in the target process. It is deliberately not the inspector's `usize`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Address(u64);

impl Address {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, offset: u64) -> Result<Self> {
        self.0
            .checked_add(offset)
            .map(Self)
            .ok_or(Error::AddressOverflow)
    }

    pub fn validate_for(self, architecture: Architecture) -> Result<Self> {
        if architecture.pointer_width() == Some(PointerWidth::Bits32) && self.0 > u32::MAX as u64 {
            return Err(Error::AddressOutOfRange {
                address: self.0,
                architecture,
            });
        }
        Ok(self)
    }
}

impl From<u32> for Address {
    fn from(value: u32) -> Self {
        Self(u64::from(value))
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pointer32(u32);

impl Pointer32 {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn address(self) -> Address {
        Address::new(self.0 as u64)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pointer64(u64);

impl Pointer64 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn address(self) -> Address {
        Address::new(self.0)
    }
}
