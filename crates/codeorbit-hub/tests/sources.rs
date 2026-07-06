//! 数据源管理与运行时资源端点集成测试

use std::path::PathBuf;
use std::sync::{Arc, Once};

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tokio::sync::RwLock;
use tower::ServiceExt;

use codeorbit_hub::api::AppState;
use codeorbit_hub::{HubState, config_installer, router, source_service};

const TOKEN: &str = "src-test-token-1234567890abcdef";

static INIT: Once = Once::new();

/// 一次性设置环境：bundled 插件目录、隔离的用户目录、带假 bridge 的运行时目录
fn setup() {
    INIT.call_once(|| {
        let bundled = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("bundled-plugins");
        let home = std::env::temp_dir().join("codeorbit-src-test-home");
        let runtime = std::env::temp_dir().join("codeorbit-src-test-runtime");
        let _ = std::fs::create_dir_all(&home);
        let _ = std::fs::create_dir_all(&runtime);
        // 写入假 bridge 文件，让 install 通过"运行时已就位"检查
        let bridge_name = if cfg!(windows) {
            "codeorbit-bridge.exe"
        } else {
            "codeorbit-bridge"
        };
        let _ = std::fs::write(runtime.join(bridge_name), b"fake-bridge");

        // SAFETY: 在任何测试体运行前一次性设置进程环境变量
        unsafe {
            std::env::set_var("CodeOrbit_BUNDLED_PLUGINS_DIR", &bundled);
            std::env::set_var("CodeOrbit_TEST_USERPROFILE", &home);
            std::env::set_var("CodeOrbit_RUNTIME_DIR", &runtime);
        }
    });
}

fn build_with_state() -> (axum::Router, Arc<RwLock<HubState>>) {
    let state = Arc::new(RwLock::new(HubState::new()));
    (router(AppState::new(state.clone(), TOKEN, true)), state)
}

fn build() -> axum::Router {
    build_with_state().0
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .header("Authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    send(app, req).await
}

async fn post(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("Authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    send(app, req).await
}

async fn send(app: &axum::Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn lists_all_bundled_sources() {
    setup();
    let app = build();
    let (status, body) = get(&app, "/api/sources").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 19, "应列出全部 19 个 bundled 源");
    // 按 displayName 排序
    assert!(arr[0]["capabilities"]["hookInstall"].as_bool().unwrap());
    assert_eq!(arr[0]["sourceType"], "bundled");
}

#[tokio::test]
async fn source_status_known_and_unknown() {
    setup();
    let app = build();

    let (status, body) = get(&app, "/api/sources/cursor").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["supported"], true);
    assert_eq!(body["source"], "cursor");

    // 别名端点
    let (status, body2) = get(&app, "/api/sources/cursor/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body2["supported"], true);

    let (status, body3) = get(&app, "/api/sources/definitely-not-real").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body3["supported"], false);
}

#[tokio::test]
async fn unknown_source_install_returns_400() {
    setup();
    let app = build();
    let (status, body) = post(&app, "/api/sources/definitely-not-real/install").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], false);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Unsupported source")
    );
}

#[tokio::test]
async fn unknown_source_wsl_install_returns_400() {
    setup();
    let app = build();
    let (status, body) = post(&app, "/api/sources/definitely-not-real/wsl/install").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], false);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("Unsupported source")
    );
}

#[tokio::test]
async fn source_operation_broadcasts_operation_result() {
    setup();
    let (app, state) = build_with_state();
    let mut rx = state.read().await.subscribe();

    let (status, body) = post(&app, "/api/sources/definitely-not-real/install").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], false);

    let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.event_type, "source.statusChanged");
    let data = event.data.unwrap();
    assert!(data.is_object());
    assert_eq!(data["source"], "definitely-not-real");
    assert_eq!(data["success"], false);
}

#[tokio::test]
async fn runtime_assets_and_repair_all() {
    setup();
    let app = build();

    let (status, body) = get(&app, "/api/runtime-assets").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["bridgeExePath"]
            .as_str()
            .unwrap()
            .contains("codeorbit-bridge")
    );

    let (status, body) = post(&app, "/api/sources/repair-all").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["success"].is_boolean());
}

#[tokio::test]
async fn install_uninstall_lifecycle() {
    setup();
    // 直接调用 service 验证完整生命周期（flat 格式的 cursor）
    let result = source_service::install("cursor");
    assert!(result.success, "install 应成功: {}", result.message);
    assert!(config_installer::is_plugin_installed("cursor"));

    let uninstall = source_service::uninstall("cursor");
    assert!(uninstall.success, "uninstall 应成功: {}", uninstall.message);
    assert!(!config_installer::is_plugin_installed("cursor"));
}

#[tokio::test]
async fn install_uninstall_idempotent() {
    setup();
    // 重复安装/卸载应幂等（用 trae，避免与 cursor 生命周期测试争用配置文件）
    assert!(source_service::install("trae").success);
    assert!(source_service::install("trae").success, "重复安装应幂等");
    assert!(config_installer::is_plugin_installed("trae"));

    assert!(source_service::uninstall("trae").success);
    assert!(source_service::uninstall("trae").success, "重复卸载应幂等");
    assert!(!config_installer::is_plugin_installed("trae"));
}
