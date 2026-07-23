use crate::Architecture;
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
        address: u64,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write memory at {address:#x}")]
    WriteFailed {
        address: u64,
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

    #[error("address arithmetic overflowed")]
    AddressOverflow,

    #[error("address {address:#x} is out of range for target architecture {architecture:?}")]
    AddressOutOfRange {
        address: u64,
        architecture: Architecture,
    },

    #[error("could not determine target process architecture")]
    ArchitectureQueryFailed {
        #[source]
        source: std::io::Error,
    },

    #[error("target pointer width is unknown for architecture {architecture:?}")]
    UnknownPointerWidth { architecture: Architecture },
}

/// Result type alias for procmod-core operations.
pub type Result<T> = std::result::Result<T, Error>;
