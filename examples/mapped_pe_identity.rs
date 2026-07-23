use procmod_core::{
    hash_canonical_section, hash_mapped_section, read_mapped_pe, Architecture, Process,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() -> procmod_core::Result<()> {
    let pid = std::env::args()
        .nth(1)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_else(|| {
            eprintln!("usage: mapped_pe_identity <process-id>");
            std::process::exit(2);
        });
    let process = Process::attach_read_only(pid)?;
    if process.architecture() != Architecture::X86 {
        eprintln!("target is not x86: {:?}", process.architecture());
        std::process::exit(1);
    }

    for module in process.modules()?.into_iter().filter(|module| {
        matches!(
            module.name.to_ascii_lowercase().as_str(),
            "hl.exe" | "hw.dll" | "client.dll" | "client_original.dll" | "tfc.dll"
        )
    }) {
        let identity = read_mapped_pe(&process, module.base)?;
        println!(
            "{} base={:#010x} mapped={:#x} machine={:#06x} timestamp={:#010x} image={:#x} checksum={:#010x}",
            module.name,
            module.base.value(),
            module.size,
            identity.machine,
            identity.timestamp,
            identity.image_size,
            identity.checksum
        );
        for section in identity
            .sections
            .iter()
            .filter(|section| section.immutable())
        {
            let digest = hash_mapped_section(&process, section)?;
            let canonical = hash_canonical_section(&process, &identity, section)?;
            println!(
                "  {} r{}x{} address={:#010x} virtual={:#x} mapped={:#x} sha256={} canonical={}",
                section.name,
                if section.readable { "+" } else { "-" },
                if section.executable { "+" } else { "-" },
                section.address.value(),
                section.virtual_size,
                section.mapped_size,
                hex(&digest),
                hex(&canonical)
            );
        }
    }
    Ok(())
}
