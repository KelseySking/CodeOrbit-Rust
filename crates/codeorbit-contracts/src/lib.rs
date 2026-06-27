//! CodeOrbit API 契约和数据传输对象 (DTO)
//!
//! 此 crate 定义所有 REST API 和 WebSocket 事件的数据结构，
//! 确保与 C# 版本的 JSON 序列化格式兼容。

pub mod dto;

pub use dto::*;
