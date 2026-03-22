use procmod_core::Process;

#[test]
fn attach_self() {
    let pid = std::process::id();
    let process = Process::attach(pid).unwrap();
    assert_eq!(process.pid(), pid);
}

#[test]
fn attach_nonexistent() {
    let result = Process::attach(999_999_999);
    assert!(result.is_err());
}

#[test]
fn read_own_memory() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let value: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let address = &value as *const u64 as usize;

    let read_value: u64 = unsafe { process.read(address).unwrap() };
    assert_eq!(read_value, 0xDEAD_BEEF_CAFE_BABE);
}

#[test]
fn read_bytes_own_memory() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let data: [u8; 4] = [0x11, 0x22, 0x33, 0x44];
    let address = data.as_ptr() as usize;

    let read_data = process.read_bytes(address, 4).unwrap();
    assert_eq!(read_data, &[0x11, 0x22, 0x33, 0x44]);
}

#[test]
fn read_typed_values() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let f: f32 = 3.14;
    let address = &f as *const f32 as usize;
    let read_f: f32 = unsafe { process.read(address).unwrap() };
    assert!((read_f - 3.14).abs() < f32::EPSILON);

    let i: i32 = -42;
    let address = &i as *const i32 as usize;
    let read_i: i32 = unsafe { process.read(address).unwrap() };
    assert_eq!(read_i, -42);
}

#[test]
fn read_array() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let arr: [f32; 3] = [1.0, 2.0, 3.0];
    let address = arr.as_ptr() as usize;
    let read_arr: [f32; 3] = unsafe { process.read(address).unwrap() };
    assert_eq!(read_arr, [1.0, 2.0, 3.0]);
}

#[test]
fn read_zero_bytes() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let result = process.read_bytes(0x1000, 0).unwrap();
    assert!(result.is_empty());
}

#[test]
fn read_invalid_address() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let result = process.read_bytes(0xDEAD, 4);
    assert!(result.is_err());
}

#[test]
fn write_typed_value() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let mut value: u64 = 0xAAAA_BBBB_CCCC_DDDD;
    let address = &mut value as *mut u64 as usize;

    process.write(address, &0x1122_3344_5566_7788u64).unwrap();
    assert_eq!(value, 0x1122_3344_5566_7788);
}

#[test]
fn write_bytes_own_memory() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let mut data: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
    let address = data.as_mut_ptr() as usize;

    process
        .write_bytes(address, &[0xAA, 0xBB, 0xCC, 0xDD])
        .unwrap();
    assert_eq!(data, [0xAA, 0xBB, 0xCC, 0xDD]);
}

#[test]
fn write_zero_bytes() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    process.write_bytes(0x1000, &[]).unwrap();
}

#[test]
fn write_then_read() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let mut value: f32 = 0.0;
    let address = &mut value as *mut f32 as usize;

    process.write(address, &99.5f32).unwrap();
    let read_back: f32 = unsafe { process.read(address).unwrap() };
    assert!((read_back - 99.5).abs() < f32::EPSILON);
}

#[test]
fn write_invalid_address() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let result = process.write_bytes(0xDEAD, &[0x90]);
    assert!(result.is_err());
}

#[test]
fn regions_self() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let regions = process.regions().unwrap();
    assert!(!regions.is_empty());

    for region in &regions {
        assert!(region.size > 0);
    }

    let has_readable = regions.iter().any(|r| r.protection.read);
    assert!(has_readable);

    let has_executable = regions.iter().any(|r| r.protection.execute);
    assert!(has_executable);
}

#[test]
fn modules_self() {
    let pid = std::process::id();
    let process = match Process::attach(pid) {
        Ok(p) => p,
        Err(_) => {
            eprintln!("skipping: cannot attach to self");
            return;
        }
    };

    let modules = process.modules().unwrap();
    assert!(!modules.is_empty());

    for module in &modules {
        assert!(module.base > 0);
        assert!(!module.name.is_empty());
        assert!(!module.path.is_empty());
    }

    let has_nonzero_size = modules.iter().any(|m| m.size > 0);
    assert!(has_nonzero_size);
}

#[test]
fn protection_display() {
    use procmod_core::Protection;

    let rwx = Protection {
        read: true,
        write: true,
        execute: true,
    };
    assert_eq!(format!("{}", rwx), "rwx");

    let r_only = Protection {
        read: true,
        write: false,
        execute: false,
    };
    assert_eq!(format!("{}", r_only), "r--");

    let none = Protection {
        read: false,
        write: false,
        execute: false,
    };
    assert_eq!(format!("{}", none), "---");
}
