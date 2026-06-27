//! 本地 API Token 管理 — 生成、持久化、验证

use codeorbit_core::services::SettingsManager;

/// 设置文件中存储 token 的键
pub const SETTINGS_KEY: &str = "api_token";

/// 确保存在有效 token：已有（长度 >= 32）则复用，否则生成并持久化
pub fn ensure_token(settings: &mut SettingsManager) -> String {
    let existing: String = settings.get(SETTINGS_KEY, String::new());
    if existing.len() >= 32 {
        return existing;
    }
    let token = generate_token();
    settings.set(SETTINGS_KEY, &token);
    token
}

/// 生成 32 字节随机 token，base64url（无填充）编码
fn generate_token() -> String {
    let bytes: [u8; 32] = rand::random();
    base64url_no_pad(&bytes)
}

/// 标准 base64 → URL 安全、去除填充（对齐 C# 实现）
fn base64url_no_pad(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 63) as usize] as char);
        }
    }
    out.replace('+', "-").replace('/', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_url_safe_and_long() {
        let token = generate_token();
        assert!(
            token.len() >= 40,
            "32 字节 base64 应 >= 43 字符: {}",
            token.len()
        );
        assert!(!token.contains('+') && !token.contains('/') && !token.contains('='));
    }

    #[test]
    fn ensure_token_persists_and_reuses() {
        let dir = std::env::temp_dir().join(format!("codeorbit-token-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut settings = SettingsManager::new(Some(dir.clone()));
        let t1 = ensure_token(&mut settings);
        let t2 = ensure_token(&mut settings);
        assert_eq!(t1, t2, "二次调用应复用已存 token");

        let mut reloaded = SettingsManager::new(Some(dir.clone()));
        assert_eq!(ensure_token(&mut reloaded), t1, "重载后应读到同一 token");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
