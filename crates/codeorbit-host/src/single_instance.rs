//! 单实例锁 — 基于端口的文件锁（fs2），防止同端口重复启动

use std::fs::{File, OpenOptions};

use fs2::FileExt;

/// 锁守卫：持有期间保持排他锁，drop 时自动释放
pub struct InstanceLock {
    _file: File,
}

/// 尝试获取指定端口的单实例锁；已被占用返回 None
pub fn acquire(port: u16) -> Option<InstanceLock> {
    let path = std::env::temp_dir().join(format!("codeorbit-{port}.lock"));
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .ok()?;
    match file.try_lock_exclusive() {
        Ok(()) => Some(InstanceLock { _file: file }),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_same_port_fails() {
        // 使用一个不常见端口避免与真实实例冲突
        let port = 59123;
        let first = acquire(port);
        assert!(first.is_some(), "首次获取应成功");
        let second = acquire(port);
        assert!(second.is_none(), "同端口二次获取应失败");

        drop(first);
        // 释放后可再次获取
        let third = acquire(port);
        assert!(third.is_some(), "释放后应可再次获取");
    }

    #[test]
    fn different_ports_can_coexist() {
        let a = acquire(59124);
        let b = acquire(59125);
        assert!(a.is_some() && b.is_some(), "不同端口应可并存");
    }
}
