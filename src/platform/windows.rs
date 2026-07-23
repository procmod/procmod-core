use crate::error::{Error, Result};
use crate::module::Module;
use crate::region::{MemoryRegion, Protection};
use crate::Architecture;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_EXECUTE, PAGE_EXECUTE_READ,
    PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
};
use windows_sys::Win32::System::ProcessStatus::{
    EnumProcessModulesEx, GetModuleBaseNameW, GetModuleFileNameExW, GetModuleInformation,
    LIST_MODULES_ALL, MODULEINFO,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

const IMAGE_FILE_MACHINE_UNKNOWN: u16 = 0;
const IMAGE_FILE_MACHINE_I386: u16 = 0x014c;
const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xaa64;

extern "system" {
    fn IsWow64Process2(process: HANDLE, process_machine: *mut u16, native_machine: *mut u16)
        -> i32;

    fn ReadProcessMemory(
        process: HANDLE,
        base_address: *const c_void,
        buffer: *mut c_void,
        size: usize,
        bytes_read: *mut usize,
    ) -> i32;

    fn WriteProcessMemory(
        process: HANDLE,
        base_address: *mut c_void,
        buffer: *const c_void,
        size: usize,
        bytes_written: *mut usize,
    ) -> i32;
}

pub struct ProcessHandle {
    handle: HANDLE,
}

#[derive(Clone, Copy)]
pub enum Access {
    ReadOnly,
    ReadWrite,
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

pub fn attach(pid: u32, access: Access) -> Result<ProcessHandle> {
    let rights = match access {
        Access::ReadOnly => PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
        Access::ReadWrite => {
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION
        }
    };
    let handle = unsafe { OpenProcess(rights, 0, pid) };

    if handle.is_null() {
        let err = std::io::Error::last_os_error();
        let raw = err.raw_os_error().unwrap_or(0);
        if raw == 5 {
            return Err(Error::PermissionDenied { pid });
        }
        if raw == 87 {
            return Err(Error::ProcessNotFound { pid });
        }
        return Err(Error::AttachFailed { pid, source: err });
    }

    Ok(ProcessHandle { handle })
}

pub fn architecture(handle: &ProcessHandle) -> Result<Architecture> {
    let mut process_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    let mut native_machine = IMAGE_FILE_MACHINE_UNKNOWN;
    if unsafe { IsWow64Process2(handle.handle, &mut process_machine, &mut native_machine) } == 0 {
        return Err(Error::ArchitectureQueryFailed {
            source: std::io::Error::last_os_error(),
        });
    }

    let machine = if process_machine == IMAGE_FILE_MACHINE_UNKNOWN {
        native_machine
    } else {
        process_machine
    };
    Ok(match machine {
        IMAGE_FILE_MACHINE_I386 => Architecture::X86,
        IMAGE_FILE_MACHINE_AMD64 => Architecture::X86_64,
        IMAGE_FILE_MACHINE_ARM64 => Architecture::Arm64,
        other => Architecture::Unknown(other),
    })
}

pub fn read_bytes(handle: &ProcessHandle, address: usize, buf: &mut [u8]) -> Result<()> {
    if buf.is_empty() {
        return Ok(());
    }

    let mut bytes_read: usize = 0;
    let result = unsafe {
        ReadProcessMemory(
            handle.handle,
            address as *const c_void,
            buf.as_mut_ptr() as *mut c_void,
            buf.len(),
            &mut bytes_read,
        )
    };

    if result == 0 {
        return Err(Error::ReadFailed {
            address: address as u64,
            source: std::io::Error::last_os_error(),
        });
    }

    if bytes_read != buf.len() {
        return Err(Error::ReadFailed {
            address: address as u64,
            source: std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!(
                    "partial read: expected {} bytes, got {}",
                    buf.len(),
                    bytes_read
                ),
            ),
        });
    }

    Ok(())
}

pub fn write_bytes(handle: &ProcessHandle, address: usize, buf: &[u8]) -> Result<()> {
    if buf.is_empty() {
        return Ok(());
    }

    let mut bytes_written: usize = 0;
    let result = unsafe {
        WriteProcessMemory(
            handle.handle,
            address as *mut c_void,
            buf.as_ptr() as *const c_void,
            buf.len(),
            &mut bytes_written,
        )
    };

    if result == 0 {
        return Err(Error::WriteFailed {
            address: address as u64,
            source: std::io::Error::last_os_error(),
        });
    }

    if bytes_written != buf.len() {
        return Err(Error::WriteFailed {
            address: address as u64,
            source: std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                format!(
                    "partial write: expected {} bytes, wrote {}",
                    buf.len(),
                    bytes_written
                ),
            ),
        });
    }

    Ok(())
}

fn page_protection_to_flags(protect: u32) -> Protection {
    match protect {
        PAGE_READONLY => Protection {
            read: true,
            write: false,
            execute: false,
        },
        PAGE_READWRITE | PAGE_WRITECOPY => Protection {
            read: true,
            write: true,
            execute: false,
        },
        PAGE_EXECUTE => Protection {
            read: false,
            write: false,
            execute: true,
        },
        PAGE_EXECUTE_READ => Protection {
            read: true,
            write: false,
            execute: true,
        },
        PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY => Protection {
            read: true,
            write: true,
            execute: true,
        },
        _ => Protection {
            read: false,
            write: false,
            execute: false,
        },
    }
}

pub fn regions(handle: &ProcessHandle, _pid: u32) -> Result<Vec<MemoryRegion>> {
    let mut result = Vec::new();
    let mut address: usize = 0;

    loop {
        let mut info = unsafe { std::mem::zeroed::<MEMORY_BASIC_INFORMATION>() };
        let written = unsafe {
            VirtualQueryEx(
                handle.handle,
                address as *const _,
                &mut info,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };

        if written == 0 {
            break;
        }

        if info.State == MEM_COMMIT {
            result.push(MemoryRegion {
                base: crate::Address::new(info.BaseAddress as usize as u64),
                size: info.RegionSize,
                protection: page_protection_to_flags(info.Protect),
            });
        }

        address = info.BaseAddress as usize + info.RegionSize;
    }

    Ok(result)
}

pub fn modules(handle: &ProcessHandle, _pid: u32) -> Result<Vec<Module>> {
    let mut h_modules: [HANDLE; 1024] = [std::ptr::null_mut(); 1024];
    let mut cb_needed: u32 = 0;

    let result = unsafe {
        EnumProcessModulesEx(
            handle.handle,
            h_modules.as_mut_ptr(),
            std::mem::size_of_val(&h_modules) as u32,
            &mut cb_needed,
            LIST_MODULES_ALL,
        )
    };

    if result == 0 {
        return Err(Error::ModuleEnumFailed {
            source: std::io::Error::last_os_error(),
        });
    }

    let count = (cb_needed as usize / std::mem::size_of::<HANDLE>()).min(h_modules.len());
    let mut modules = Vec::with_capacity(count);

    for &h_module in &h_modules[..count] {
        let mut name_buf = [0u16; 260];
        let name_len = unsafe {
            GetModuleBaseNameW(
                handle.handle,
                h_module,
                name_buf.as_mut_ptr(),
                name_buf.len() as u32,
            )
        };

        let name = if name_len > 0 {
            String::from_utf16_lossy(&name_buf[..name_len as usize])
        } else {
            String::new()
        };

        let mut mod_info = unsafe { std::mem::zeroed::<MODULEINFO>() };
        let info_result = unsafe {
            GetModuleInformation(
                handle.handle,
                h_module,
                &mut mod_info,
                std::mem::size_of::<MODULEINFO>() as u32,
            )
        };

        let (base, size) = if info_result != 0 {
            (mod_info.lpBaseOfDll as usize, mod_info.SizeOfImage as usize)
        } else {
            (0, 0)
        };

        let mut path_buf = [0u16; 260];
        let path_len = unsafe {
            GetModuleFileNameExW(
                handle.handle,
                h_module,
                path_buf.as_mut_ptr(),
                path_buf.len() as u32,
            )
        };

        let path = if path_len > 0 {
            String::from_utf16_lossy(&path_buf[..path_len as usize])
        } else {
            name.clone()
        };

        modules.push(Module {
            name,
            base: crate::Address::new(base as u64),
            size,
            path,
        });
    }

    Ok(modules)
}
