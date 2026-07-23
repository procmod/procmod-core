use procmod_core::Process;

fn main() -> procmod_core::Result<()> {
    let argument = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: read_only_modules <process-id>");
        std::process::exit(2);
    });
    let pid = argument.parse::<u32>().unwrap_or_else(|_| {
        eprintln!("process-id must be a decimal integer");
        std::process::exit(2);
    });

    let process = Process::attach_read_only(pid)?;
    println!("pid: {pid}");
    println!("architecture: {:?}", process.architecture());
    for module in process.modules()? {
        println!(
            "{:#010x} {:#010x} {} {}",
            module.base.value(),
            module.size,
            module.name,
            module.path
        );
    }
    Ok(())
}
