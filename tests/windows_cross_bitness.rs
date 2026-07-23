#![cfg(all(windows, target_arch = "x86_64"))]

use procmod_core::{Address, Architecture, Pointer32, PointerWidth, Process};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn reads_a_32_bit_child_from_a_64_bit_inspector() {
    let Some(executable) = std::env::var_os("PROC_MOD_X86_CHILD") else {
        return;
    };
    let mut child = ChildGuard(
        Command::new(executable)
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to start 32-bit child"),
    );
    let stdout = child.0.stdout.take().unwrap();
    let mut line = String::new();
    BufReader::new(stdout).read_line(&mut line).unwrap();
    let addresses: Vec<u32> = line
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect();

    let process = Process::attach_read_only(child.0.id()).unwrap();
    assert_eq!(process.architecture(), Architecture::X86);
    assert_eq!(
        process.architecture().pointer_width(),
        Some(PointerWidth::Bits32)
    );
    assert!(process.modules().unwrap().iter().all(|module| {
        module.base.value() <= u64::from(u32::MAX)
            && module
                .base
                .checked_add(module.size.saturating_sub(1) as u64)
                .is_ok_and(|last| last.value() <= u64::from(u32::MAX))
    }));

    let pointer: Pointer32 = unsafe { process.read_at(Address::from(addresses[0])).unwrap() };
    assert_eq!(pointer.address(), Address::from(addresses[1]));
    let value: u32 = unsafe { process.read_at(pointer.address()).unwrap() };
    assert_eq!(value, 0x5a17_c0de);

    let error = process
        .read_bytes_at(Address::new(u64::from(u32::MAX) + 1), 1)
        .unwrap_err();
    assert!(matches!(
        error,
        procmod_core::Error::AddressOutOfRange { .. }
    ));
}
