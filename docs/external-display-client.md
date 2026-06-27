# 外部展示客户端快速开始

[English](external-display-client.en.md) | [文档索引](README_CN.md)

这份文档面向想在 WPF HUD 之外开发 CodeOrbit 展示端的开发者，例如 Web UI、插件、终端面板、硬件桥接器或后续移动端 companion。

稳定边界是 [展示端合同](runtime-display-contract.zh-CN.md) 中描述的本地 Runtime API；完整 endpoint、DTO 和 WebSocket 事件参考见 [API Reference 中文](api-reference.md)；桌面、Web、移动端、插件、硬件屏幕等集成形态参考 [集成指南](integration-guide.md)。展示端应该依赖 REST DTO、WebSocket 事件和操作端点，不应该依赖 WPF ViewModel、Hub 具体类、Named Pipe hook 处理、transcript 文件或 source-specific 响应构造器。

## Runtime 连接

Runtime 默认监听本机地址：

```text
http://127.0.0.1:32145
```

WPF HUD 在 managed 模式下会启动 `codeorbit-host`，并把本地 API token 写入：

```text
%APPDATA%\CodeOrbit\settings.json
```

token 字段是：

```json
{
  "api_token": "..."
}
```

独立 Runtime 开发时，可以显式指定 token：

```bash
cargo run -p codeorbit-host -- --token dev-token --port 32145
```

然后在展示客户端中使用同一个 token。

## 认证

`GET /api/health` 不需要认证。其它端点都需要 Runtime token。

REST 请求优先使用 header：

```http
Authorization: Bearer <api_token>
```

WebSocket 客户端可以把 token 放到 query string：

```text
ws://127.0.0.1:32145/api/events?token=<api_token>
```

不要把 token 写入日志，也不要持久化到用户本机设置以外的位置。除非另立配对/认证设计任务，否则不要把 API 暴露到局域网或公网地址。

## 最小客户端流程

1. 用 `GET /api/health` 检查 Runtime 是否存活。
2. 用 `GET /api/capabilities` 读取能力声明。
3. 获取初始快照：
   - `GET /api/sessions`
   - `GET /api/pending`
   - 如果 UI 展示 hook/source 状态，再调用 `GET /api/sources`。
4. 连接 `WS /api/events`。
5. WebSocket 重连后先重新获取快照，再处理后续事件。
6. 遇到未知事件类型时记录并继续运行。对已知的 `session.*`、`pending.*` 或 `source.*` 事件族，如果客户端没有完整理解 payload，就重新获取对应 REST 快照。

## Pending 操作

pending action id 来自 `GET /api/pending`。

允许或拒绝权限请求：

```http
POST /api/permissions/{actionId}/allow
Content-Type: application/json

{ "always": false }
```

```http
POST /api/permissions/{actionId}/deny
Content-Type: application/json

{ "reason": "denied from external display" }
```

按当前可见步骤回答问题：

```http
POST /api/questions/{actionId}/answer-current
Content-Type: application/json

{ "answers": ["selected-value-or-free-text"] }
```

`answer-current` 返回：

```json
{
  "success": true,
  "resolved": false
}
```

当 `resolved` 为 `false` 时，保留这个 pending action，并重新获取 `/api/pending`；Runtime 已经推进到下一个问题步骤。当 `resolved` 为 `true` 时，下一次 pending 快照会移除这个 action。

关闭问题：

```http
POST /api/questions/{actionId}/dismiss
```

只有当客户端一次性拥有完整 keyed answer map 时，才使用 `POST /api/questions/{actionId}/answer`。HUD 风格的逐步问答展示端应使用 `answer-current`。

## 重连与兼容性

展示端应该把 Runtime 状态视为服务端状态：

* 启动和 WebSocket 重连后重新获取 `/api/sessions` 与 `/api/pending`。
* 容忍未知 session status、source id、DTO 字段和事件类型。
* 把 nullable 字段视为可选展示提示。
* selected row、动画、主题等 UI-only 状态留在展示端内部。
* 如果展示端需要 Runtime 当前没有暴露的数据，应扩展公共 contract，而不是读取 WPF、Hub、Core、Bridge、transcript 或 settings 内部实现。


