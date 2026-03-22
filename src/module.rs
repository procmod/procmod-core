/// A loaded module (shared library, executable, or dylib) in a process.
#[derive(Debug, Clone)]
pub struct Module {
    /// Module file name (e.g., "libfoo.so").
    pub name: String,

    /// Base address where the module is loaded.
    pub base: usize,

    /// Size of the module in memory (bytes).
    pub size: usize,

    /// Full file path on disk.
    pub path: String,
}
