//! 命令行参数

use clap::Parser;

/// CodeOrbit RuntimeHost — 核心服务进程
#[derive(Debug, Parser)]
#[command(name = "codeorbit-host", version, about)]
pub struct Args {
    /// 设置文件目录路径
    #[arg(long)]
    pub settings_dir: Option<String>,

    /// HTTP 服务端口
    #[arg(long, default_value_t = 32145)]
    pub port: u16,

    /// HTTP 服务绑定地址
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// API 认证令牌（缺省时自动生成/复用）
    #[arg(long)]
    pub token: Option<String>,

    /// IPC 管道名称
    #[arg(long)]
    pub pipe_name: Option<String>,

    /// 拥有者进程 PID
    #[arg(long)]
    pub owner_pid: Option<u32>,

    /// 拥有者退出时自动关闭
    #[arg(long, default_value_t = false)]
    pub shutdown_when_owner_exits: bool,

    /// 跳过启动时的源修复
    #[arg(long, default_value_t = false)]
    pub no_repair: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_applied() {
        let args = Args::parse_from(["codeorbit-host"]);
        assert_eq!(args.port, 32145);
        assert_eq!(args.host, "127.0.0.1");
        assert!(!args.no_repair);
        assert!(args.token.is_none());
    }

    #[test]
    fn parses_all_flags() {
        let args = Args::parse_from([
            "codeorbit-host",
            "--port",
            "40000",
            "--host",
            "0.0.0.0",
            "--token",
            "secret",
            "--pipe-name",
            "mypipe",
            "--owner-pid",
            "1234",
            "--shutdown-when-owner-exits",
            "--no-repair",
        ]);
        assert_eq!(args.port, 40000);
        assert_eq!(args.host, "0.0.0.0");
        assert_eq!(args.token.as_deref(), Some("secret"));
        assert_eq!(args.pipe_name.as_deref(), Some("mypipe"));
        assert_eq!(args.owner_pid, Some(1234));
        assert!(args.shutdown_when_owner_exits);
        assert!(args.no_repair);
    }
}
