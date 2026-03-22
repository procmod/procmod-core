use crate::error::{Error, Result};
use crate::module::Module;
use crate::region::{MemoryRegion, Protection};
use std::ffi::c_void;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::Memory::{
    VirtualQueryEx, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_EXECUTE, PAGE_EXECUTE_READ,
    PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
};
use windows_sys::Win32::System::ProcessStatus::{
    EnumProcessModulesEx, GetModuleBaseNameW, GetModuleInformation, LIST_MODULES_ALL, MODULEINFO,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

extern "system" {
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

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

pub fn attach(pid: u32) -> Result<ProcessHandle> {
    let access =
        PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION;
    let handle = unsafe { OpenProcess(access, 0, pid) };

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
            address,
            source: std::io::Error::last_os_error(),
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
            address,
            source: std::io::Error::last_os_error(),
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
                base: info.BaseAddress as usize,
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

    let count = cb_needed as usize / std::mem::size_of::<HANDLE>();
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

        modules.push(Module {
            name: name.clone(),
            base,
            size,
            path: name,
        });
    }

    Ok(modules)
}
