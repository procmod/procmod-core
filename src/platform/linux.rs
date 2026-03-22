use crate::error::{Error, Result};
use crate::module::Module;
use crate::region::{MemoryRegion, Protection};
use std::collections::HashMap;

pub struct ProcessHandle {
    pid: i32,
}

pub fn attach(pid: u32) -> Result<ProcessHandle> {
    let proc_path = format!("/proc/{}", pid);
    if !std::path::Path::new(&proc_path).exists() {
        return Err(Error::ProcessNotFound { pid });
    }
    Ok(ProcessHandle { pid: pid as i32 })
}

pub fn read_bytes(handle: &ProcessHandle, address: usize, buf: &mut [u8]) -> Result<()> {
    if buf.is_empty() {
        return Ok(());
    }

    let local_iov = libc::iovec {
        iov_base: buf.as_mut_ptr() as *mut _,
        iov_len: buf.len(),
    };
    let remote_iov = libc::iovec {
        iov_base: address as *mut _,
        iov_len: buf.len(),
    };

    let result = unsafe { libc::process_vm_readv(handle.pid, &local_iov, 1, &remote_iov, 1, 0) };

    if result == -1 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EPERM) {
            return Err(Error::PermissionDenied {
                pid: handle.pid as u32,
            });
        }
        return Err(Error::ReadFailed {
            address,
            source: err,
        });
    }

    if (result as usize) != buf.len() {
        return Err(Error::ReadFailed {
            address,
            source: std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("partial read: expected {} bytes, got {}", buf.len(), result),
            ),
        });
    }

    Ok(())
}

pub fn write_bytes(handle: &ProcessHandle, address: usize, buf: &[u8]) -> Result<()> {
    if buf.is_empty() {
        return Ok(());
    }

    let local_iov = libc::iovec {
        iov_base: buf.as_ptr() as *mut _,
        iov_len: buf.len(),
    };
    let remote_iov = libc::iovec {
        iov_base: address as *mut _,
        iov_len: buf.len(),
    };

    let result = unsafe { libc::process_vm_writev(handle.pid, &local_iov, 1, &remote_iov, 1, 0) };

    if result == -1 {
        return Err(Error::WriteFailed {
            address,
            source: std::io::Error::last_os_error(),
        });
    }

    if (result as usize) != buf.len() {
        return Err(Error::WriteFailed {
            address,
            source: std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                format!(
                    "partial write: expected {} bytes, wrote {}",
                    buf.len(),
                    result
                ),
            ),
        });
    }

    Ok(())
}

fn parse_protection(perms: &str) -> Protection {
    let bytes = perms.as_bytes();
    Protection {
        read: bytes.first() == Some(&b'r'),
        write: bytes.get(1) == Some(&b'w'),
        execute: bytes.get(2) == Some(&b'x'),
    }
}

fn read_maps(pid: i32) -> std::result::Result<String, std::io::Error> {
    let path = format!("/proc/{}/maps", pid);
    std::fs::read_to_string(&path)
}

pub fn regions(handle: &ProcessHandle, _pid: u32) -> Result<Vec<MemoryRegion>> {
    let content = read_maps(handle.pid).map_err(|e| Error::RegionQueryFailed { source: e })?;
    let mut result = Vec::new();

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let range = match parts.next() {
            Some(r) => r,
            None => continue,
        };
        let perms = match parts.next() {
            Some(p) => p,
            None => continue,
        };

        let (start_str, end_str) = match range.split_once('-') {
            Some(pair) => pair,
            None => continue,
        };

        let start = usize::from_str_radix(start_str, 16).unwrap_or(0);
        let end = usize::from_str_radix(end_str, 16).unwrap_or(0);

        result.push(MemoryRegion {
            base: start,
            size: end - start,
            protection: parse_protection(perms),
        });
    }

    Ok(result)
}

pub fn modules(handle: &ProcessHandle, _pid: u32) -> Result<Vec<Module>> {
    let content = read_maps(handle.pid).map_err(|e| Error::ModuleEnumFailed { source: e })?;

    // group mapped file regions by path to build module entries
    let mut module_map: HashMap<String, (usize, usize)> = HashMap::new();

    for line in content.lines() {
        let mut parts = line.split_whitespace();
        let range = match parts.next() {
            Some(r) => r,
            None => continue,
        };

        // skip perms, offset, dev, inode
        for _ in 0..4 {
            parts.next();
        }

        let path = match parts.next() {
            Some(p) if p.starts_with('/') => p,
            _ => continue,
        };

        let (start_str, end_str) = match range.split_once('-') {
            Some(pair) => pair,
            None => continue,
        };

        let start = usize::from_str_radix(start_str, 16).unwrap_or(0);
        let end = usize::from_str_radix(end_str, 16).unwrap_or(0);

        let entry = module_map.entry(path.to_string()).or_insert((start, end));
        entry.0 = entry.0.min(start);
        entry.1 = entry.1.max(end);
    }

    let mut result: Vec<Module> = module_map
        .into_iter()
        .map(|(path, (base, end))| {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            Module {
                name,
                base,
                size: end - base,
                path,
            }
        })
        .collect();

    result.sort_by_key(|m| m.base);
    Ok(result)
}
