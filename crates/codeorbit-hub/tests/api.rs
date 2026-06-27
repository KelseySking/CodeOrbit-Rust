//! REST API 集成测试 — 端点路由、认证、错误路径

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use tokio::sync::RwLock;
use tower::ServiceExt;

use codeorbit_core::models::HookEvent;
use codeorbit_hub::api::AppState;
use codeorbit_hub::{HubState, router};

const TOKEN: &str = "secret-test-token-1234567890";

fn build() -> (axum::Router, Arc<RwLock<HubState>>) {
    let state = Arc::new(RwLock::new(HubState::new()));
    let app_state = AppState::new(state.clone(), TOKEN, true);
    (router(app_state), state)
}

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        builder = builder.header("Authorization", format!("Bearer {t}"));
    }
    let req = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };

    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let value: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

fn session_start(session_id: &str) -> HookEvent {
    HookEvent {
        event_name: "SessionStart".to_string(),
        session_id: Some(session_id.to_string()),
        tool_name: None,
        tool_use_id: None,
        agent_id: None,
        tool_input: None,
        raw_json: json!({ "hook_event_name": "SessionStart" }),
        source: Some("claude".to_string()),
        parent_pid: None,
        tracked_pid: None,
        tracked_pid_kind: None,
        tracked_process_started_at_utc: None,
    }
}

#[tokio::test]
async fn health_is_public() {
    let (app, _) = build();
    let (status, body) = send(&app, "GET", "/api/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn protected_route_requires_token() {
    let (app, _) = build();
    let (status, body) = send(&app, "GET", "/api/sessions", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn bearer_token_authorizes() {
    let (app, _) = build();
    let (status, body) = send(&app, "GET", "/api/sessions", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn header_and_query_tokens_authorize() {
    let (app, _) = build();

    // X-CodeOrbit-Token
    let req = Request::builder()
        .method("GET")
        .uri("/api/sessions")
        .header("X-CodeOrbit-Token", TOKEN)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // query param
    let (status, _) = send(
        &app,
        "GET",
        &format!("/api/sessions?token={TOKEN}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn sessions_listed_and_dismissed() {
    let (app, state) = build();
    state.write().await.handle_event(&session_start("s1"));

    let (status, body) = send(&app, "GET", "/api/sessions", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["sessionId"], "s1");

    let (status, body) = send(&app, "GET", "/api/sessions/s1", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["sessionId"], "s1");

    let (status, body) = send(&app, "POST", "/api/sessions/s1/dismiss", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);

    let (status, _) = send(&app, "GET", "/api/sessions/s1", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_session_returns_404() {
    let (app, _) = build();
    let (status, body) = send(&app, "GET", "/api/sessions/nope", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
}

#[tokio::test]
async fn pending_endpoints_work() {
    let (app, _) = build();

    let (status, body) = send(&app, "GET", "/api/pending", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 0);

    let (status, body) = send(&app, "GET", "/api/pending/history", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["entries"].is_array());
}

#[tokio::test]
async fn permission_allow_on_unknown_returns_404() {
    let (app, _) = build();
    let (status, body) = send(
        &app,
        "POST",
        "/api/permissions/permission-xyz/allow",
        Some(TOKEN),
        Some(json!({ "always": false })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
}

#[tokio::test]
async fn capabilities_reports_localhost_mode() {
    let (app, _) = build();
    let (status, body) = send(&app, "GET", "/api/capabilities", Some(TOKEN), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["securityMode"], "localhost-token");
    assert_eq!(body["approval"], true);
}
