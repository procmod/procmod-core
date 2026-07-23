#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(not(target_os = "windows"))]
#[derive(Clone, Copy)]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

#[cfg(not(target_os = "windows"))]
pub fn architecture(_handle: &ProcessHandle) -> crate::Result<crate::Architecture> {
    Ok(if cfg!(target_arch = "x86") {
        crate::Architecture::X86
    } else if cfg!(target_arch = "x86_64") {
        crate::Architecture::X86_64
    } else if cfg!(target_arch = "aarch64") {
        crate::Architecture::Arm64
    } else {
        crate::Architecture::Unknown(0)
    })
}
