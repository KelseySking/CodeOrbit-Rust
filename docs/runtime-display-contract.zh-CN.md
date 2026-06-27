# CodeOrbit Runtime 展示端合同

[English](runtime-display-contract.md) | [文档索引](README_CN.md)

本文档定义 CodeOrbit Runtime 和所有展示端之间的职责边界。Runtime 是 CodeOrbit 的中心化控制面：它接入已配置 CLI source 的 hook 活动，归一化 session 和 pending action 状态，并把同一份状态暴露给一个或多个展示端。展示端可以是当前 WPF HUD、Web UI、移动端、硬件屏幕或第三方集成。

当前 Windows HUD 启动或连接 `codeorbit-host`，并通过公开 REST/WebSocket contract 消费 Runtime 状态。Runtime 源码已拆到独立的 `CodeOrbit-Runtime` 仓库；Windows 仓库保留为官方 WPF 展示客户端。

开发入口：

- [外部展示端快速开始](external-display-client.md)：连接 Runtime、订阅事件、发送 pending action 操作。
- [完整 API 文档](api-reference.md)：REST endpoint、DTO、WebSocket 事件。
- [其他应用集成方式](integration-guide.md)：桌面、Web、手机、插件、硬件屏幕等集成方式。

## 职责边界

### Runtime 负责

Runtime 是所有 CLI 和 agent 活动的事实来源：

- 通过 Bridge 和 Named Pipe 协议接入 CLI hook。
- 集中聚合宿主设备上已配置的 CLI source，并为未来远程设备 source adapter 留扩展空间。
- source-specific hook 安装、修复、能力声明和 Runtime 资产管理。
- source/event 归一化和 source-specific hook response 格式化。
- session 生命周期、工具历史、最近消息、transcript 消费和进程清理。
- pending permission 和 question 队列。
- permission/question 解析，包括超时行为和写回 CLI hook 的 response JSON。
- 本地 REST API、WebSocket event stream、auth token 和能力声明。
- 多客户端实时事件广播。多个展示端可以同时连接，并观察同一份 Runtime 状态。

### 展示端负责

展示端是可替换的呈现层：

- 通过 REST 拉取初始 Runtime 状态。
- 通过 WebSocket 订阅 Runtime 变化。
- 渲染 sessions、pending actions、source status 和 runtime assets。
- 通过 REST 把用户操作发回 Runtime，例如 allow、deny、answer、dismiss、activate terminal。
- 保存展示端本地状态，例如选中项、窗口位置、动画状态、主题、密度和音效偏好。
- 可选地管理本地 Runtime 进程生命周期，但这只是运行层职责。展示端不能拥有 Runtime 业务状态。

展示端不得实现 hook ingestion、session reducer、pending action queue、source 安装变更或 source-specific hook response 格式化。如果展示端需要 API 未暴露的数据，应扩展公开 API contract，而不是读取 Runtime 内部实现。

## 拓扑模式

Runtime 支持两类拓扑：

- 本地 managed 模式：官方桌面展示端在 `127.0.0.1` 启动 Runtime，连接它，并在退出时只关闭自己拥有的 Runtime 进程。
- 共享远程模式：Runtime 显式绑定到非 loopback host，例如 `0.0.0.0`，允许局域网或其他设备上的多个展示端携带 token 连接。这种模式下展示端可以为方便而启动 Runtime，但展示端退出时不应关闭 Runtime，因为其他客户端可能仍然连接。

Runtime 默认必须保持 localhost-only。远程/移动端访问需要用户显式配置、token 认证，以及后续 pairing/security 加固。不要静默绑定到局域网地址。

## 数据流

```text
AI CLI hook
  -> codeorbit-bridge
  -> Runtime hook server
  -> Runtime session/pending state
  -> REST snapshots + WebSocket events
  -> Display client
  -> REST action calls
  -> Runtime resolves pending hook response
  -> codeorbit-bridge stdout
```

WPF HUD 既是展示端，也可能启动 managed local Runtime 进程。这只是运行层职责；WPF 不得拥有 hook 处理或 Runtime 状态业务逻辑。

## 认证

`GET /api/health` 不需要认证，用于 liveness check。其他 API route 都需要 Runtime token。客户端可以使用：

- `Authorization: Bearer <token>`
- `X-CodeOrbit-Token: <token>`
- `?token=<token>` query 参数

query token 主要用于简单 WebSocket 客户端和本地工具。UI 客户端应优先使用 header。

## REST Endpoint

所有 route 都位于 `/api` 下。完整请求/响应见 [API Reference 中文](api-reference.md)。

| 类别 | Endpoint |
| --- | --- |
| Runtime | `GET /health`, `GET /version`, `GET /capabilities` |
| Sources | `GET /sources`, `GET /sources/{source}`, `GET /sources/{source}/status`, `POST /sources/{source}/install`, `POST /sources/{source}/uninstall`, `POST /sources/{source}/repair`, `POST /sources/repair-all` |
| Runtime assets | `GET /runtime-assets`, `POST /runtime-assets/repair` |
| Sessions | `GET /sessions`, `GET /sessions/{sessionId}`, `GET /sessions/{sessionId}/messages`, `POST /sessions/{sessionId}/dismiss`, `POST /sessions/{sessionId}/activate-terminal` |
| Pending actions | `GET /pending`, `GET /pending/{actionId}`, `POST /permissions/{actionId}/allow`, `POST /permissions/{actionId}/deny`, `POST /questions/{actionId}/answer`, `POST /questions/{actionId}/answer-current`, `POST /questions/{actionId}/dismiss` |
| Realtime | `WS /events?token=<token>` |

## WebSocket Events

客户端用相同 token 规则连接 `/api/events`。服务端发送 `HubEventDto`：

```json
{
  "type": "session.updated",
  "timestampUtc": "2026-06-12T00:00:00Z",
  "data": {}
}
```

已知事件：

| Type | Data | 客户端行为 |
| --- | --- | --- |
| `session.updated` | `SessionDto[]` | 替换或 reconcile session list。 |
| `session.removed` | `{ sessionId: string }` | 本地移除 session，或重新拉取 `/sessions`。 |
| `pending.updated` | `PendingActionDto[]` | 替换或 reconcile pending list。 |
| `pending.resolved` | `{ actionId: string, pending: PendingActionDto[] }` | 移除已解决 action 并 reconcile pending list。 |
| `source.statusChanged` | `SourceOperationResultDto` 或 `SourceDto[]` | 重新拉取 `/sources` 获取规范化 source snapshot。 |

展示端必须容忍未知事件类型。多个 WebSocket client 可同时连接；Runtime 会向所有授权客户端广播事件。断线重连后用 REST snapshot 恢复状态。

## DTO 兼容规则

`codeorbit-contracts` 中的 DTO 是公开展示端合同：

- 默认只做 additive nullable 字段变更。
- 不应修改已有字段名和语义，除非有 capability flag 或替代 endpoint。
- 暴露给客户端的日期时间字段必须使用 UTC `DateTimeOffset`。
- 状态类字段使用 string，客户端必须处理未知值。
- 错误响应应使用带稳定 `code` 的 `ApiErrorDto`。
- WebSocket event 应携带 snapshot-friendly payload，客户端能在 reconnect 后通过 REST 重新恢复。

## 当前迁移缺口

- WPF 仍可能为 selected-session refresh 读取 transcript。Runtime 应最终拥有 transcript 消费并暴露 display-ready messages。
- `source.statusChanged` 当前可能有多种 payload shape。展示端应重新拉取 `/sources`，直到事件 payload 规范化。
- WPF HUD 状态和操作已经消费 Runtime REST/WebSocket contract；设置 `runtime_launch_mode=external` 可连接已运行的 standalone RuntimeHost。
- 远程/移动端访问不是默认开启能力。Runtime 已有 host binding knob，但 pairing、token rotation UX、CORS/browser-hosting policy 和 remote-device source ingestion 需要后续任务。
