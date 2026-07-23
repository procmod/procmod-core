use crate::error::Result;
use crate::module::Module;
use crate::platform;
use crate::region::MemoryRegion;
use crate::{Address, Architecture, Pointer32, Pointer64, PointerWidth};

mod sealed {
    pub trait Sealed {}
}

pub trait Capability: sealed::Sealed {
    const ACCESS: platform::Access;
}

pub struct ReadOnly;
pub struct ReadWrite;

impl sealed::Sealed for ReadOnly {}
impl sealed::Sealed for ReadWrite {}

impl Capability for ReadOnly {
    const ACCESS: platform::Access = platform::Access::ReadOnly;
}

impl Capability for ReadWrite {
    const ACCESS: platform::Access = platform::Access::ReadWrite;
}

/// A handle to an external process with an explicit memory-access capability.
pub struct Process<C: Capability = ReadWrite> {
    inner: platform::ProcessHandle,
    pid: u32,
    architecture: Architecture,
    capability: std::marker::PhantomData<C>,
}

impl Process<ReadWrite> {
    /// Attach with read and write access.
    pub fn attach(pid: u32) -> Result<Self> {
        Self::attach_with(pid)
    }

    /// Write a typed value to the target process at the given address.
    pub fn write<T: Copy>(&self, address: Address, value: &T) -> Result<()> {
        let buf = unsafe {
            std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
        };
        platform::write_bytes(&self.inner, self.native_address(address)?, buf)
    }

    /// Write raw bytes to the target process.
    pub fn write_bytes(&self, address: Address, bytes: &[u8]) -> Result<()> {
        platform::write_bytes(&self.inner, self.native_address(address)?, bytes)
    }
}

impl Process<ReadOnly> {
    /// Attach with only the operating-system rights required to query and read.
    pub fn attach_read_only(pid: u32) -> Result<Self> {
        Self::attach_with(pid)
    }
}

impl<C: Capability> Process<C> {
    fn attach_with(pid: u32) -> Result<Self> {
        let inner = platform::attach(pid, C::ACCESS)?;
        let architecture = platform::architecture(&inner)?;
        Ok(Self {
            inner,
            pid,
            architecture,
            capability: std::marker::PhantomData,
        })
    }

    /// Returns the PID of the attached process.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn architecture(&self) -> Architecture {
        self.architecture
    }

    /// Read a typed value from a target-width-independent address.
    ///
    /// # Safety
    ///
    /// `T` must be valid for any bit pattern.
    pub unsafe fn read_at<T: Copy>(&self, address: Address) -> Result<T> {
        let mut value = std::mem::MaybeUninit::<T>::uninit();
        let buf =
            std::slice::from_raw_parts_mut(value.as_mut_ptr() as *mut u8, std::mem::size_of::<T>());
        platform::read_bytes(&self.inner, self.native_address(address)?, buf)?;
        Ok(value.assume_init())
    }

    pub fn read_bytes_at(&self, address: Address, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        platform::read_bytes(&self.inner, self.native_address(address)?, &mut buf)?;
        Ok(buf)
    }

    /// Read a pointer using the attached target's pointer width.
    pub fn read_pointer(&self, address: Address) -> Result<Address> {
        match self.architecture.pointer_width() {
            Some(PointerWidth::Bits32) => {
                let pointer: Pointer32 = unsafe { self.read_at(address)? };
                Ok(pointer.address())
            }
            Some(PointerWidth::Bits64) => {
                let pointer: Pointer64 = unsafe { self.read_at(address)? };
                Ok(pointer.address())
            }
            None => Err(crate::Error::UnknownPointerWidth {
                architecture: self.architecture,
            }),
        }
    }

    /// Read a typed value from the target process at the given address.
    ///
    /// # Safety
    ///
    /// `T` must be valid for any bit pattern. Primitive numeric types (`u8`, `i32`,
    /// `f32`, `[u8; N]`, etc.) are safe. Types with validity invariants (`bool`,
    /// `char`, enums, references) will cause undefined behavior if the remote
    /// memory contains an invalid representation.
    pub unsafe fn read<T: Copy>(&self, address: Address) -> Result<T> {
        self.read_at(address)
    }

    /// Read raw bytes from the target process.
    pub fn read_bytes(&self, address: Address, len: usize) -> Result<Vec<u8>> {
        self.read_bytes_at(address, len)
    }

    fn native_address(&self, address: Address) -> Result<usize> {
        let address = address.validate_for(self.architecture)?.value();
        usize::try_from(address).map_err(|_| crate::Error::AddressOutOfRange {
            address,
            architecture: self.architecture,
        })
    }

    /// List all loaded modules in the target process.
    pub fn modules(&self) -> Result<Vec<Module>> {
        let modules = platform::modules(&self.inner, self.pid)?;
        Ok(modules
            .into_iter()
            .filter(|module| {
                let Some(last_byte) = module.size.checked_sub(1) else {
                    return false;
                };
                let Ok(last_byte) = u64::try_from(last_byte) else {
                    return false;
                };
                module.base.validate_for(self.architecture).is_ok()
                    && module
                        .base
                        .checked_add(last_byte)
                        .and_then(|address| address.validate_for(self.architecture))
                        .is_ok()
            })
            .collect())
    }

    /// List all memory regions in the target process.
    pub fn regions(&self) -> Result<Vec<MemoryRegion>> {
        platform::regions(&self.inner, self.pid)
    }
}
