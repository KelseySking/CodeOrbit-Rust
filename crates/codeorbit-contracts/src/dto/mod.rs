//! API 数据传输对象 (DTO)
//!
//! 所有类型使用 `#[serde(rename_all = "camelCase")]` 以匹配 C# 的 JSON 格式。

mod event;
mod health;
mod pending;
mod permission;
mod question;
mod session;
mod source;

pub use event::*;
pub use health::*;
pub use pending::*;
pub use permission::*;
pub use question::*;
pub use session::*;
pub use source::*;
