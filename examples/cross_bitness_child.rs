#[cfg(windows)]
fn main() {
    use std::io::Write;

    let value = Box::new(0x5a17_c0de_u32);
    let value_address = (&*value as *const u32 as usize) as u32;
    let pointer = Box::new(value_address);
    let pointer_address = (&*pointer as *const u32 as usize) as u32;

    println!("{pointer_address} {value_address}");
    std::io::stdout().flush().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(60));
}

#[cfg(not(windows))]
fn main() {}
