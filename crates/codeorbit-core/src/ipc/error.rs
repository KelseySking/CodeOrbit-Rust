//! IPC 错误类型

use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("payload too large: {size} bytes (max {max})")]
    PayloadTooLarge { size: u32, max: u32 },

    #[error("protocol violation: {0}")]
    ProtocolViolation(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("connection closed")]
    ConnectionClosed,
}

pub type IpcResult<T> = Result<T, IpcError>;
