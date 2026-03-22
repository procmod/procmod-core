<p align="center">
<img src="logo.svg" width="256" height="256" alt="procmod-core">
</p>

<h1 align="center">procmod-core</h1>

<p align="center">Cross-platform process memory read/write for Rust.</p>

---

Read and write memory in external processes on macOS, Linux, and Windows. Enumerate loaded modules and memory regions. Pure Rust, no C FFI wrappers.

## Install

```toml
[dependencies]
procmod-core = "1"
```

## Quick start

Read a game's player health from a known memory address:

```rust
use procmod_core::Process;

fn main() -> procmod_core::Result<()> {
    let game = Process::attach(pid)?;

    // read the player's health at a known offset
    let health: f32 = unsafe { game.read(0x7FF6_1A00_4200)? };
    println!("player health: {}", health);

    Ok(())
}
```

## Usage

### Attach to a process

```rust
let process = Process::attach(1234)?;
```

### Read and write memory

```rust
// read a typed value (T must be valid for any bit pattern)
let hp: f32 = unsafe { process.read(address)? };

// write a typed value
process.write(address, &100.0_f32)?;

// raw byte operations
let bytes = process.read_bytes(address, 64)?;
process.write_bytes(address, &[0x90, 0x90, 0x90])?;
```

### Enumerate modules

Find where a game's main executable or a specific DLL is loaded, then scan from its base address:

```rust
let modules = process.modules()?;
for m in &modules {
    println!("{}: base={:#x} size={:#x}", m.name, m.base, m.size);
}

// find a specific module
let engine = modules.iter().find(|m| m.name == "engine.dll").unwrap();
let scan_region = process.read_bytes(engine.base, engine.size)?;
```

### Query memory regions

Understand what memory is mapped and with what permissions - useful for finding writable data segments or executable code:

```rust
let regions = process.regions()?;
for r in &regions {
    println!("{:#x} ({} bytes) {}", r.base, r.size, r.protection);
}

// find all writable regions
let writable: Vec<_> = regions.iter().filter(|r| r.protection.write).collect();
```

## Platform support

| Platform | Backend | Architectures |
|----------|---------|---------------|
| macOS | Mach VM (`mach_vm_read_overwrite` / `mach_vm_write`) | x86_64, arm64 |
| Linux | `process_vm_readv` / `process_vm_writev` | x86_64, arm64 |
| Windows | `ReadProcessMemory` / `WriteProcessMemory` | x86_64 |

## Permissions

- **macOS**: Requires the `com.apple.security.cs.debugger` entitlement or running as root. SIP must allow task_for_pid on the target.
- **Linux**: Requires `CAP_SYS_PTRACE` or appropriate `ptrace_scope` settings. Reading a child process's memory generally works without extra privileges.
- **Windows**: Requires `SeDebugPrivilege` for system processes. Standard user can read/write processes they own.

## License

MIT
