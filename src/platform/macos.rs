use crate::error::{Error, Result};
use crate::module::Module;
use crate::region::{MemoryRegion, Protection};
use std::ffi::c_int;

type MachPortT = u32;
type KernReturnT = c_int;
type VmProtT = c_int;
type MachMsgTypeNumberT = u32;

const KERN_SUCCESS: KernReturnT = 0;
const KERN_INVALID_ADDRESS: KernReturnT = 1;
const VM_PROT_READ: VmProtT = 1;
const VM_PROT_WRITE: VmProtT = 2;
const VM_PROT_EXECUTE: VmProtT = 4;
const VM_REGION_BASIC_INFO_64: c_int = 9;
const VM_REGION_BASIC_INFO_64_COUNT: MachMsgTypeNumberT = 9;
const TASK_DYLD_INFO: u32 = 17;
const TASK_DYLD_INFO_COUNT: MachMsgTypeNumberT = 5;
const MH_MAGIC_64: u32 = 0xFEEDFACF;
const LC_SEGMENT_64: u32 = 0x19;

#[repr(C)]
struct VmRegionBasicInfo64 {
    protection: VmProtT,
    max_protection: VmProtT,
    inheritance: u32,
    shared: i32,
    reserved: i32,
    offset: u64,
    behavior: i32,
    user_wired_count: u16,
}

#[repr(C)]
struct TaskDyldInfo {
    all_image_info_addr: u64,
    all_image_info_size: u64,
    all_image_info_format: c_int,
}

extern "C" {
    fn mach_task_self() -> MachPortT;

    fn task_for_pid(target: MachPortT, pid: c_int, task: *mut MachPortT) -> KernReturnT;

    fn mach_vm_read_overwrite(
        task: MachPortT,
        address: u64,
        size: u64,
        data: u64,
        out_size: *mut u64,
    ) -> KernReturnT;

    fn mach_vm_write(task: MachPortT, address: u64, data: *const u8, count: u32) -> KernReturnT;

    fn mach_vm_region(
        task: MachPortT,
        address: *mut u64,
        size: *mut u64,
        flavor: c_int,
        info: *mut VmRegionBasicInfo64,
        count: *mut MachMsgTypeNumberT,
        object_name: *mut MachPortT,
    ) -> KernReturnT;

    fn mach_port_deallocate(task: MachPortT, name: MachPortT) -> KernReturnT;

    fn task_info(
        target_task: MachPortT,
        flavor: u32,
        task_info_out: *mut TaskDyldInfo,
        task_info_count: *mut MachMsgTypeNumberT,
    ) -> KernReturnT;
}

pub struct ProcessHandle {
    task: MachPortT,
    owns_port: bool,
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        if self.owns_port {
            unsafe {
                mach_port_deallocate(mach_task_self(), self.task);
            }
        }
    }
}

pub fn attach(pid: u32) -> Result<ProcessHandle> {
    let self_port = unsafe { mach_task_self() };
    let self_pid = std::process::id();

    if pid == self_pid {
        return Ok(ProcessHandle {
            task: self_port,
            owns_port: false,
        });
    }

    let mut task: MachPortT = 0;
    let kr = unsafe { task_for_pid(self_port, pid as c_int, &mut task) };

    if kr != KERN_SUCCESS {
        let err = std::io::Error::from_raw_os_error(kr);
        if kr == 5 {
            return Err(Error::PermissionDenied { pid });
        }
        return Err(Error::AttachFailed { pid, source: err });
    }

    Ok(ProcessHandle {
        task,
        owns_port: true,
    })
}

pub fn read_bytes(handle: &ProcessHandle, address: usize, buf: &mut [u8]) -> Result<()> {
    if buf.is_empty() {
        return Ok(());
    }

    let mut out_size: u64 = 0;
    let kr = unsafe {
        mach_vm_read_overwrite(
            handle.task,
            address as u64,
            buf.len() as u64,
            buf.as_mut_ptr() as u64,
            &mut out_size,
        )
    };

    if kr != KERN_SUCCESS {
        return Err(Error::ReadFailed {
            address,
            source: std::io::Error::from_raw_os_error(kr),
        });
    }

    Ok(())
}

pub fn write_bytes(handle: &ProcessHandle, address: usize, buf: &[u8]) -> Result<()> {
    if buf.is_empty() {
        return Ok(());
    }

    let kr = unsafe { mach_vm_write(handle.task, address as u64, buf.as_ptr(), buf.len() as u32) };

    if kr != KERN_SUCCESS {
        return Err(Error::WriteFailed {
            address,
            source: std::io::Error::from_raw_os_error(kr),
        });
    }

    Ok(())
}

pub fn regions(handle: &ProcessHandle, _pid: u32) -> Result<Vec<MemoryRegion>> {
    let mut result = Vec::new();
    let mut address: u64 = 0;

    loop {
        let mut size: u64 = 0;
        let mut info = unsafe { std::mem::zeroed::<VmRegionBasicInfo64>() };
        let mut count = VM_REGION_BASIC_INFO_64_COUNT;
        let mut object_name: MachPortT = 0;

        let kr = unsafe {
            mach_vm_region(
                handle.task,
                &mut address,
                &mut size,
                VM_REGION_BASIC_INFO_64,
                &mut info,
                &mut count,
                &mut object_name,
            )
        };

        if kr != KERN_SUCCESS {
            if kr == KERN_INVALID_ADDRESS {
                break;
            }
            return Err(Error::RegionQueryFailed {
                source: std::io::Error::from_raw_os_error(kr),
            });
        }

        result.push(MemoryRegion {
            base: address as usize,
            size: size as usize,
            protection: Protection {
                read: info.protection & VM_PROT_READ != 0,
                write: info.protection & VM_PROT_WRITE != 0,
                execute: info.protection & VM_PROT_EXECUTE != 0,
            },
        });

        address += size;
    }

    Ok(result)
}

pub fn modules(handle: &ProcessHandle, _pid: u32) -> Result<Vec<Module>> {
    let mut dyld_info = unsafe { std::mem::zeroed::<TaskDyldInfo>() };
    let mut count = TASK_DYLD_INFO_COUNT;

    let kr = unsafe { task_info(handle.task, TASK_DYLD_INFO, &mut dyld_info, &mut count) };

    if kr != KERN_SUCCESS {
        return Err(Error::ModuleEnumFailed {
            source: std::io::Error::from_raw_os_error(kr),
        });
    }

    // dyld_all_image_infos layout (64-bit):
    // offset 0:  version (u32)
    // offset 4:  infoArrayCount (u32)
    // offset 8:  infoArray pointer (u64)
    let mut header_buf = [0u8; 16];
    read_bytes(
        handle,
        dyld_info.all_image_info_addr as usize,
        &mut header_buf,
    )?;

    let info_array_count = u32::from_ne_bytes(header_buf[4..8].try_into().unwrap()) as usize;
    let info_array_ptr = u64::from_ne_bytes(header_buf[8..16].try_into().unwrap());

    // dyld_image_info is 24 bytes: load_address(u64), file_path(u64), mod_date(u64)
    let array_size = info_array_count * 24;
    let mut array_buf = vec![0u8; array_size];
    read_bytes(handle, info_array_ptr as usize, &mut array_buf)?;

    let mut result = Vec::with_capacity(info_array_count);

    for i in 0..info_array_count {
        let offset = i * 24;
        let load_address = u64::from_ne_bytes(array_buf[offset..offset + 8].try_into().unwrap());
        let file_path_ptr =
            u64::from_ne_bytes(array_buf[offset + 8..offset + 16].try_into().unwrap());

        let path = read_c_string(handle, file_path_ptr as usize, 1024).unwrap_or_default();
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();

        let size = read_macho_size(handle, load_address as usize).unwrap_or(0);

        result.push(Module {
            name,
            base: load_address as usize,
            size,
            path,
        });
    }

    Ok(result)
}

fn read_c_string(handle: &ProcessHandle, address: usize, max_len: usize) -> Result<String> {
    let mut buf = vec![0u8; max_len];
    read_bytes(handle, address, &mut buf)?;
    let nul_pos = buf.iter().position(|&b| b == 0).unwrap_or(max_len);
    Ok(String::from_utf8_lossy(&buf[..nul_pos]).into_owned())
}

fn read_macho_size(handle: &ProcessHandle, base: usize) -> Result<usize> {
    // mach_header_64: magic(4), cputype(4), cpusubtype(4), filetype(4),
    //                 ncmds(4), sizeofcmds(4), flags(4), reserved(4) = 32 bytes
    let mut header_buf = [0u8; 32];
    read_bytes(handle, base, &mut header_buf)?;

    let magic = u32::from_ne_bytes(header_buf[0..4].try_into().unwrap());
    if magic != MH_MAGIC_64 {
        return Ok(0);
    }

    let ncmds = u32::from_ne_bytes(header_buf[16..20].try_into().unwrap()) as usize;
    let sizeofcmds = u32::from_ne_bytes(header_buf[20..24].try_into().unwrap()) as usize;

    let cmd_offset = base + 32;
    let mut cmd_buf = vec![0u8; sizeofcmds];
    read_bytes(handle, cmd_offset, &mut cmd_buf)?;

    let mut min_addr: u64 = u64::MAX;
    let mut max_addr: u64 = 0;
    let mut offset = 0;

    for _ in 0..ncmds {
        if offset + 8 > cmd_buf.len() {
            break;
        }

        let cmd = u32::from_ne_bytes(cmd_buf[offset..offset + 4].try_into().unwrap());
        let cmdsize =
            u32::from_ne_bytes(cmd_buf[offset + 4..offset + 8].try_into().unwrap()) as usize;

        if cmdsize == 0 {
            break;
        }

        // segment_command_64: cmd(4), cmdsize(4), segname(16), vmaddr(8), vmsize(8)
        if cmd == LC_SEGMENT_64 && offset + 48 <= cmd_buf.len() {
            let vmaddr = u64::from_ne_bytes(cmd_buf[offset + 24..offset + 32].try_into().unwrap());
            let vmsize = u64::from_ne_bytes(cmd_buf[offset + 32..offset + 40].try_into().unwrap());

            if vmsize > 0 {
                min_addr = min_addr.min(vmaddr);
                max_addr = max_addr.max(vmaddr + vmsize);
            }
        }

        offset += cmdsize;
    }

    if max_addr > min_addr {
        Ok((max_addr - min_addr) as usize)
    } else {
        Ok(0)
    }
}
