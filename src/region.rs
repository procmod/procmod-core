/// A contiguous region of virtual memory in a process.
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Base address of the region.
    pub base: usize,

    /// Size of the region in bytes.
    pub size: usize,

    /// Memory protection flags.
    pub protection: Protection,
}

/// Memory protection flags for a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Protection {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl std::fmt::Display for Protection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
            if self.read { "r" } else { "-" },
            if self.write { "w" } else { "-" },
            if self.execute { "x" } else { "-" },
        )
    }
}
