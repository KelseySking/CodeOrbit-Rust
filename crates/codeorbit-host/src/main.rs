//! CodeOrbit RuntimeHost — 核心服务进程入口与启动编排

mod args;
mod manifest;
mod owner_monitor;
mod single_instance;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use clap::Parser;
use tokio::sync::{Notify, RwLock};

use codeorbit_core::ipc::{OVERRIDE_ENV, full_path};
use codeorbit_core::models::PermissionRequest;
use codeorbit_core::services::SettingsManager;
use codeorbit_hub::api::ensure_token;
use codeorbit_hub::{
    AppState, AutoApprove, HookServer, HubState, ProcessMonitor, config_installer, router,
    source_service,
};

const DEFAULT_PORT: u16 = 32145;
const IDLE_SESSION_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    run(args::Args::parse()).await
}

async fn run(args: args::Args) -> Result<()> {
    // 5.2 加载设置与 manifest
    let mut settings = SettingsManager::new(args.settings_dir.clone().map(PathBuf::from));
    let manifest = manifest::load(&config_installer::runtime_directory());

    // 解析端口/地址/令牌/管道名
    let port = resolve_port(&args, manifest.as_ref(), &settings);
    let host = normalize_host(&args.host);
    let token = match args.token.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => t.to_string(),
        _ => ensure_token(&mut settings),
    };
    if let Some(pipe) = args
        .pipe_name
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        // SAFETY: 启动早期、单线程阶段设置进程环境变量
        unsafe {
            std::env::set_var(OVERRIDE_ENV, pipe);
        }
    }

    // 5.3 单实例锁
    let _lock = match single_instance::acquire(port) {
        Some(lock) => lock,
        None => {
            eprintln!("CodeOrbit RuntimeHost 已在端口 {port} 运行（无法获取单实例锁）");
            std::process::exit(1);
        }
    };

    // 5.4 可选修复所有源
    if !args.no_repair {
        let _ = source_service::repair_all();
    }

    // 5.5 创建 HubState（含自动审批）
    let auto_approve_enabled = settings.get("auto_approve_safe_tools", false);
    let auto_approve: AutoApprove = Box::new(move |req: &PermissionRequest| {
        auto_approve_enabled
            && matches!(
                req.tool_name.as_str(),
                "Read" | "Grep" | "Glob" | "LS" | "TodoRead"
            )
    });
    let state = Arc::new(RwLock::new(HubState::with_auto_approve(Some(auto_approve))));

    let session_timeout =
        Duration::from_secs(settings.get("session_timeout", 300_i64).clamp(30, 3600) as u64);

    let shutdown = Arc::new(Notify::new());

    // 5.6 启动 IPC HookServer
    let hook_handle = tokio::spawn(HookServer::run(state.clone(), session_timeout));

    // 5.8 进程监控 + 空闲清理（每秒）
    let monitor_state = state.clone();
    let monitor_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            ProcessMonitor::scan_once(&monitor_state).await;
            monitor_state.write().await.remove_idle_sessions(
                IDLE_SESSION_TIMEOUT,
                Utc::now(),
                "session idle timeout",
            );
        }
    });

    // 3. Owner 进程监控
    if args.shutdown_when_owner_exits
        && let Some(owner_pid) = args.owner_pid
    {
        tokio::spawn(owner_monitor::watch(owner_pid, shutdown.clone()));
    }

    // 5.7 Axum HTTP 服务器（实时事件经 HubState broadcast → WS，无需额外桥接）
    let app = router(AppState::new(state.clone(), token, is_loopback(&host)));
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(
        "CodeOrbit RuntimeHost 监听 http://{addr}  pipe={}",
        full_path()
    );

    // 6. 优雅关闭
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown.clone()))
        .await?;

    hook_handle.abort();
    monitor_handle.abort();
    tracing::info!("CodeOrbit RuntimeHost 已关闭");
    Ok(())
}

/// 解析端口：显式 --port > manifest.default_port > settings.api_port > 默认；并钳制到合法范围
fn resolve_port(
    args: &args::Args,
    manifest: Option<&manifest::RuntimeManifest>,
    settings: &SettingsManager,
) -> u16 {
    let port = if args.port != DEFAULT_PORT {
        args.port
    } else {
        manifest
            .and_then(|m| m.default_port)
            .unwrap_or_else(|| settings.get("api_port", DEFAULT_PORT))
    };
    port.clamp(1024, 65535)
}

/// 规范化绑定地址：`*`/`+` → `0.0.0.0`，空 → `127.0.0.1`
fn normalize_host(host: &str) -> String {
    let value = host.trim();
    match value {
        "" => "127.0.0.1".to_string(),
        "*" | "+" => "0.0.0.0".to_string(),
        other => other.to_string(),
    }
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// 关闭信号：Ctrl+C / SIGTERM(Unix) / owner 退出通知
async fn shutdown_signal(notify: Arc<Notify>) {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut sig) = signal(SignalKind::terminate()) {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
        _ = notify.notified() => {}
    }
    tracing::info!("收到关闭信号，正在优雅关闭...");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_host_rules() {
        assert_eq!(normalize_host("  "), "127.0.0.1");
        assert_eq!(normalize_host("*"), "0.0.0.0");
        assert_eq!(normalize_host("+"), "0.0.0.0");
        assert_eq!(normalize_host("0.0.0.0"), "0.0.0.0");
    }

    #[test]
    fn loopback_detection() {
        assert!(is_loopback("127.0.0.1"));
        assert!(is_loopback("localhost"));
        assert!(!is_loopback("0.0.0.0"));
    }
}
