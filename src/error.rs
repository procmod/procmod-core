use thiserror::Error;

/// Errors that can occur during process memory operations.
#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to attach to process {pid}")]
    AttachFailed {
        pid: u32,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read memory at {address:#x}")]
    ReadFailed {
        address: usize,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write memory at {address:#x}")]
    WriteFailed {
        address: usize,
        #[source]
        source: std::io::Error,
    },

    #[error("process not found: {pid}")]
    ProcessNotFound { pid: u32 },

    #[error("permission denied for process {pid}")]
    PermissionDenied { pid: u32 },

    #[error("failed to enumerate modules")]
    ModuleEnumFailed {
        #[source]
        source: std::io::Error,
    },

    #[error("failed to query memory regions")]
    RegionQueryFailed {
        #[source]
        source: std::io::Error,
    },
}

/// Result type alias for procmod-core operations.
pub type Result<T> = std::result::Result<T, Error>;
