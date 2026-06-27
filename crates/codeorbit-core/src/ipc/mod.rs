//! IPC 通信层 — MessageProtocol + 跨平台传输

pub mod error;
pub mod message_protocol;
pub mod named_pipe_path;
pub mod transport;

pub use error::{IpcError, IpcResult};
pub use message_protocol::{MAX_PAYLOAD_SIZE, encode, read_message_async, write_message_async};
pub use named_pipe_path::{OVERRIDE_ENV, full_path, pipe_name};
pub use transport::{IpcClient, IpcServer, IpcStream};
