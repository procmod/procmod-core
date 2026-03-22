use crate::error::Result;
use crate::module::Module;
use crate::platform;
use crate::region::MemoryRegion;

/// A handle to an external process for memory operations.
pub struct Process {
    inner: platform::ProcessHandle,
    pid: u32,
}

impl Process {
    /// Attach to a running process by its PID.
    pub fn attach(pid: u32) -> Result<Self> {
        let inner = platform::attach(pid)?;
        Ok(Self { inner, pid })
    }

    /// Returns the PID of the attached process.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Read a typed value from the target process at the given address.
    ///
    /// # Safety
    ///
    /// `T` must be valid for any bit pattern. Primitive numeric types (`u8`, `i32`,
    /// `f32`, `[u8; N]`, etc.) are safe. Types with validity invariants (`bool`,
    /// `char`, enums, references) will cause undefined behavior if the remote
    /// memory contains an invalid representation.
    pub unsafe fn read<T: Copy>(&self, address: usize) -> Result<T> {
        let mut value = std::mem::MaybeUninit::<T>::uninit();
        let buf =
            std::slice::from_raw_parts_mut(value.as_mut_ptr() as *mut u8, std::mem::size_of::<T>());
        platform::read_bytes(&self.inner, address, buf)?;
        Ok(value.assume_init())
    }

    /// Write a typed value to the target process at the given address.
    pub fn write<T: Copy>(&self, address: usize, value: &T) -> Result<()> {
        let buf = unsafe {
            std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
        };
        platform::write_bytes(&self.inner, address, buf)
    }

    /// Read raw bytes from the target process.
    pub fn read_bytes(&self, address: usize, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        platform::read_bytes(&self.inner, address, &mut buf)?;
        Ok(buf)
    }

    /// Write raw bytes to the target process.
    pub fn write_bytes(&self, address: usize, bytes: &[u8]) -> Result<()> {
        platform::write_bytes(&self.inner, address, bytes)
    }

    /// List all loaded modules in the target process.
    pub fn modules(&self) -> Result<Vec<Module>> {
        platform::modules(&self.inner, self.pid)
    }

    /// List all memory regions in the target process.
    pub fn regions(&self) -> Result<Vec<MemoryRegion>> {
        platform::regions(&self.inner, self.pid)
    }
}
