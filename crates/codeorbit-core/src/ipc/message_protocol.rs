//! 消息协议：4 字节小端长度前缀 + UTF-8 JSON payload

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::error::{IpcError, IpcResult};

/// 最大 payload 大小：1MB
pub const MAX_PAYLOAD_SIZE: u32 = 1_048_576;

/// 编码消息：4 字节 LE 长度前缀 + UTF-8 payload
pub fn encode(json: &str) -> Vec<u8> {
    let payload = json.as_bytes();
    let len = payload.len() as u32;
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

/// 异步读取一条消息，返回 `None` 表示连接正常关闭（EOF）。
pub async fn read_message_async<R: AsyncRead + Unpin>(stream: &mut R) -> IpcResult<Option<String>> {
    // 读取 4 字节长度前缀
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            return Ok(None);
        }
        Err(e) => return Err(IpcError::Io(e)),
    }

    let length = u32::from_le_bytes(len_buf);
    if length > MAX_PAYLOAD_SIZE {
        return Err(IpcError::PayloadTooLarge {
            size: length,
            max: MAX_PAYLOAD_SIZE,
        });
    }

    // 读取 payload
    let mut payload = vec![0u8; length as usize];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(|_| IpcError::ConnectionClosed)?;

    String::from_utf8(payload)
        .map(Some)
        .map_err(|e| IpcError::ProtocolViolation(format!("invalid utf-8: {e}")))
}

/// 异步写入一条消息 + flush
pub async fn write_message_async<W: AsyncWrite + Unpin>(
    stream: &mut W,
    json: &str,
) -> IpcResult<()> {
    let encoded = encode(json);
    stream.write_all(&encoded).await?;
    stream.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_roundtrip() {
        let msg = r#"{"type":"ping"}"#;
        let buf = encode(msg);
        assert_eq!(&buf[..4], &(msg.len() as u32).to_le_bytes());
        assert_eq!(&buf[4..], msg.as_bytes());
    }

    #[tokio::test]
    async fn read_write_roundtrip() {
        let msg = r#"{"hello":"world"}"#;
        let mut buf = Vec::new();
        write_message_async(&mut buf, msg).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let result = read_message_async(&mut cursor).await.unwrap();
        assert_eq!(result, Some(msg.to_string()));
    }

    #[tokio::test]
    async fn read_eof_returns_none() {
        let mut empty: &[u8] = &[];
        let result = read_message_async(&mut empty).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn oversized_payload_rejected() {
        let big = "x".repeat(MAX_PAYLOAD_SIZE as usize + 1);
        let mut buf = Vec::new();
        // 手动编码超大消息
        let len = big.len() as u32;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(big.as_bytes());

        let mut cursor = std::io::Cursor::new(buf);
        let err = read_message_async(&mut cursor).await.unwrap_err();
        assert!(matches!(err, IpcError::PayloadTooLarge { .. }));
    }
}
