# CodeOrbit Runtime API Reference

[English](api-reference.en.md) | [文档索引](README_CN.md)

本文档面向 Windows HUD、Web、移动端、硬件屏幕和第三方展示端开发者，描述 Runtime 暴露的完整 REST/WebSocket 接口。接口源代码位于 `src/codeorbit-contracts/ApiDtos.cs` 和 `src/codeorbit-hub/CodeOrbitApiHost.cs`。

## 基本信息

默认地址：

```text
http://127.0.0.1:32145
```

所有 REST endpoint 都以 `/api` 为前缀。除 `GET /api/health` 外，其余 endpoint 都需要 Runtime token。

认证方式任选其一：

```http
Authorization: Bearer <api_token>
X-CodeOrbit-Token: <api_token>
```

WebSocket 可使用 query token：

```text
ws://127.0.0.1:32145/api/events?token=<api_token>
```

默认安全模式是 localhost token。远程访问必须显式启动：

```bash
codeorbit-host.exe --host 0.0.0.0 --port 32145 --token <token>
```

浏览器跨域、配对码、token 轮换和公网暴露策略不属于当前默认能力；Web 展示端优先使用同源代理、桌面壳或后续专门的 pairing/security 方案。

## RuntimeHost 启动参数

| 参数 | 说明 |
| --- | --- |
| `--settings-dir <path>` | 指定 settings.json 所在目录。未指定时使用 `%APPDATA%\CodeOrbit`。 |
| `--port <port>` | 指定 REST/WebSocket 端口。有效范围会被归一化到 `1024..65535`。 |
| `--host <host>` | 指定监听地址。默认 `127.0.0.1`。`0.0.0.0` 表示显式远程共享模式。 |
| `--token <token>` | 指定 API token。未指定时从 settings 读取或生成。 |
| `--pipe-name <name>` | 指定 Named Pipe 名称，主要用于测试或多实例隔离。 |
| `--owner-pid <pid>` | 指定拥有 Runtime 的前端进程 PID。 |
| `--shutdown-when-owner-exits` | owner 进程退出后 Runtime 自动退出。只适合本地私有 managed 模式。 |
| `--no-repair` | 启动时不自动修复已安装 source hook。 |

同一端口只允许一个 RuntimeHost 实例运行。第二个相同端口的 RuntimeHost 会快速失败。

## 响应约定

- 成功响应为 JSON。
- 日期字段使用 UTC `DateTimeOffset`，序列化为 ISO 8601 字符串。
- 未找到资源通常返回 `404` 和 `ApiErrorDto`。
- 未认证返回 `401` 和 `ApiErrorDto`。
- display client 必须容忍未知字段、未知 session status、未知 source id 和未知 WebSocket event type。

错误响应：

```json
{
  "code": "unauthorized",
  "message": "Missing or invalid CodeOrbit API token"
}
```

## 其他应用集成方式

如果你正在开发 Windows/Mac/Linux 桌面应用、Web UI、浏览器插件、IDE 插件、手机 companion、硬件屏幕或企业内部面板，请先阅读 [集成指南](integration-guide.md)。该文档说明每类应用应该如何启动或连接 Runtime、是否拥有 Runtime 生命周期、如何处理远程共享模式，以及如何做 Runtime 自动更新。

接口层面的统一规则是：展示端只通过本文件描述的 REST/WebSocket contract 集成 Runtime；新增 CLI source 才进入 Runtime/Core/Bridge 内部实现。
## Endpoint 总览

### Runtime

| Method | Path | 认证 | 说明 |
| --- | --- | --- | --- |
| `GET` | `/api/health` | 不需要 | Runtime 存活检查。 |
| `GET` | `/api/version` | 需要 | Runtime 产品名和版本。 |
| `GET` | `/api/capabilities` | 需要 | Runtime 能力和安全模式。 |

### Sources 和 Runtime 资产

| Method | Path | 说明 |
| --- | --- | --- |
| `GET` | `/api/sources` | 列出支持的 CLI source 和安装状态。 |
| `GET` | `/api/sources/{source}` | 获取 source 状态。 |
| `GET` | `/api/sources/{source}/status` | source 状态别名。 |
| `GET` | `/api/sources/wsl/distros` | 列出用户侧 WSL 发行版（含 state/version/default；过滤 Docker）。 |
| `GET` | `/api/sources/{source}/wsl/status?distro=<name>` | WSL 内 hook 状态（含 `probeOk`/`error`）。 |
| `POST` | `/api/sources/{source}/install` | 安装或更新某个 source 的 CodeOrbit hook。 |
| `POST` | `/api/sources/{source}/uninstall` | 卸载 CodeOrbit 自己拥有的 hook entry。 |
| `POST` | `/api/sources/{source}/repair` | 修复某个 source 的 hook 配置。 |
| `POST` | `/api/sources/{source}/wsl/install?distro=<name>` | 在 WSL 内安装调用 Windows bridge 的 source hook。 |
| `POST` | `/api/sources/{source}/wsl/uninstall?distro=<name>` | 卸载 WSL 内 CodeOrbit 自己拥有的 hook entry。 |
| `POST` | `/api/sources/{source}/wsl/repair?distro=<name>` | 修复某个 WSL source hook 配置。 |
| `POST` | `/api/sources/repair-all` | 仅修复 Windows 侧已安装 source（`scope: "windows"`）。 |
| `GET` | `/api/runtime-assets` | 获取 Runtime hook script 和 bridge 路径。 |
| `POST` | `/api/runtime-assets/repair` | 修复共享 Runtime 资产。 |

### Sessions

| Method | Path | 说明 |
| --- | --- | --- |
| `GET` | `/api/sessions` | 获取当前所有会话快照。 |
| `GET` | `/api/sessions/{sessionId}` | 获取单个会话。 |
| `GET` | `/api/sessions/{sessionId}/messages` | 获取单个会话的最近消息。 |
| `POST` | `/api/sessions/{sessionId}/dismiss` | 从 Runtime 状态中移除会话，并关闭该会话相关 pending hook。 |
| `POST` | `/api/sessions/{sessionId}/activate-terminal` | 请求激活该会话对应的终端。 |

### Pending Actions

| Method | Path | 说明 |
| --- | --- | --- |
| `GET` | `/api/pending` | 获取所有待处理审批/问答。 |
| `GET` | `/api/pending/{actionId}` | 获取单个 pending action。 |
| `POST` | `/api/permissions/{actionId}/allow` | 批准权限请求。 |
| `POST` | `/api/permissions/{actionId}/deny` | 拒绝权限请求。 |
| `POST` | `/api/questions/{actionId}/answer` | 一次性回答问题，适合完整 keyed answer map。 |
| `POST` | `/api/questions/{actionId}/answer-current` | 回答当前可见问题步骤，适合 HUD/移动端逐步问答。 |
| `POST` | `/api/questions/{actionId}/dismiss` | 关闭问题。 |

### Realtime

| Method | Path | 说明 |
| --- | --- | --- |
| `WS` | `/api/events?token=<api_token>` | WebSocket 事件流，可同时连接多个展示端。 |

## Endpoint 详情

### `GET /api/health`

不需要 token。

响应 `ApiHealthDto`：

```json
{
  "status": "ok",
  "startedAtUtc": "2026-06-15T00:00:00Z"
}
```

### `GET /api/version`

响应 `ApiVersionDto`：

```json
{
  "product": "CodeOrbit Runtime",
  "version": "1.0.1.0"
}
```

### `GET /api/capabilities`

响应 `ApiCapabilitiesDto`：

```json
{
  "hookInjection": true,
  "approval": true,
  "question": true,
  "transcript": true,
  "realtime": true,
  "realtimeProtocols": ["websocket"],
  "securityMode": "localhost-token"
}
```

`securityMode`：

| 值 | 说明 |
| --- | --- |
| `localhost-token` | Runtime 绑定 loopback 地址。默认模式。 |
| `remote-token` | Runtime 绑定非 loopback 地址。远程展示端可连接，但必须带 token。 |

### `GET /api/sources`

响应 `SourceDto[]`：

```json
[
  {
    "id": "codex",
    "displayName": "Codex",
    "iconName": "codex",
    "installed": true,
    "capabilities": {
      "hookInstall": true,
      "approval": true,
      "question": true,
      "transcript": true,
      "alwaysAllow": true
    }
  }
]
```

### `GET /api/sources/{source}` / `GET /api/sources/{source}/status`

响应 `SourceStatusDto`：

```json
{
  "source": "codex",
  "supported": true,
  "installed": true,
  "displayName": "Codex"
}
```

Windows 侧状态不含 `distro` / `probeOk` / `error`（字段省略）。

### Source install/uninstall/repair

请求体为空。

```http
POST /api/sources/codex/install
POST /api/sources/codex/uninstall
POST /api/sources/codex/repair
```

响应 `SourceOperationResultDto`：

```json
{
  "source": "codex",
  "success": true,
  "installed": true,
  "message": "codex installed"
}
```

失败时通常返回 `400`，响应体仍是 `SourceOperationResultDto`，`success=false`，并带稳定 `code`（见下表）。

### WSL source 操作

`GET /api/sources/wsl/distros` 返回**用户侧** WSL 发行版（已过滤 `docker-desktop*` 等系统 distro）：

```json
{
  "distros": [
    {
      "name": "Ubuntu",
      "state": "Running",
      "version": 2,
      "isDefault": true
    },
    {
      "name": "Debian",
      "state": "Stopped",
      "version": 2,
      "isDefault": false
    }
  ],
  "defaultDistro": "Ubuntu"
}
```

WSL 不可用时返回 `400`：`{ "distros": [], "defaultDistro": null, "message": "...", "code": "wsl_unavailable" }`。

`GET /api/sources/{source}/wsl/status?distro=Ubuntu` 返回 `SourceStatusDto`，额外字段：

```json
{
  "source": "claude",
  "supported": true,
  "installed": true,
  "displayName": "Claude Code",
  "distro": "Ubuntu",
  "probeOk": true
}
```

- `probeOk=false` 时 **不要** 把 `installed=false` 当成「未安装」；此时 HTTP `400`，并带 `error` 说明探测失败原因。
- `distro` 为实际解析到的发行版（显式 query 或默认）。

`POST /api/sources/{source}/wsl/install`、`/uninstall` 和 `/repair` 支持可选 `distro` query。未指定时 Runtime 使用默认用户侧 WSL 发行版。成功/失败响应均可能带 `distro`；失败带 `code`。

WSL hook 通过 WSL interop 调用 Windows `codeorbit-bridge.exe`，命令形如 `"/mnt/c/.../codeorbit-bridge.exe" --source <source>`。

### `POST /api/sources/repair-all`

请求体为空。

**仅修复 Windows 侧**已安装 source，**不含 WSL**。

响应：

```json
{
  "success": true,
  "scope": "windows"
}
```

### `GET /api/runtime-assets`

响应 `RuntimeAssetsDto`：

```json
{
  "runtimeDirectory": "C:\\Users\\name\\.CodeOrbit",
  "hookScriptPath": "C:\\Users\\name\\.CodeOrbit\\CodeOrbit-hook.ps1",
  "bridgeExePath": "C:\\Users\\name\\.CodeOrbit\\CodeOrbit-bridge.exe",
  "installed": true
}
```

### `POST /api/runtime-assets/repair`

请求体为空。

响应：

```json
{
  "success": true,
  "assets": {
    "runtimeDirectory": "...",
    "hookScriptPath": "...",
    "bridgeExePath": "...",
    "installed": true
  }
}
```

### `GET /api/sessions`

响应 `SessionDto[]`。展示端启动、WebSocket 重连、收到未知 session event 后应重新拉取该快照。

```json
[
  {
    "sessionId": "abc",
    "source": "codex",
    "sourceDisplayName": "Codex",
    "projectName": "CodeOrbit",
    "workingDirectory": "D:\\Work\\CodeOrbit",
    "status": "Running",
    "currentToolName": "Read",
    "currentToolDescription": "Read README.md",
    "createdAtUtc": "2026-06-15T00:00:00Z",
    "lastUpdatedAtUtc": "2026-06-15T00:01:00Z",
    "trackedPid": 12345,
    "trackedProcessStartedAtUtc": "2026-06-15T00:00:00Z",
    "lastUserPrompt": "修复构建",
    "lastAssistantMessage": "正在检查项目",
    "completionText": null,
    "transcriptPath": null,
    "transcriptPosition": 0,
    "terminalApp": "WindowsTerminal",
    "terminalSessionId": "...",
    "recentMessages": [],
    "toolHistory": []
  }
]
```

常见 `status` 值：`Idle`、`Processing`、`Running`、`WaitingQuestion`、`WaitingApproval`、`Completed`、`Error`。客户端必须容忍未来新增值。

### `GET /api/sessions/{sessionId}`

成功返回 `SessionDto`。未知 `sessionId` 返回 `404 ApiErrorDto`。

### `GET /api/sessions/{sessionId}/messages`

响应 `ChatMessageDto[]`：

```json
[
  {
    "isUser": true,
    "text": "继续任务",
    "timestampUtc": "2026-06-15T00:00:00Z"
  }
]
```

### `POST /api/sessions/{sessionId}/dismiss`

请求体为空。成功：

```json
{
  "success": true
}
```

注意：这是 Runtime 级移除，会清理相关 pending hook。普通 UI 里的“隐藏一条列表项”应优先做成展示端本地状态，不要误用此 endpoint。

### `POST /api/sessions/{sessionId}/activate-terminal`

请求体为空。成功：

```json
{
  "success": true
}
```

Runtime 会触发终端激活请求。具体是否能激活取决于当前宿主/展示端能力。

### `GET /api/pending`

响应 `PendingActionDto[]`：

```json
[
  {
    "actionId": "permission-...",
    "kind": "permission",
    "sessionId": "abc",
    "source": "codex",
    "sourceDisplayName": "Codex",
    "projectName": "CodeOrbit",
    "workingDirectory": "D:\\Work\\CodeOrbit",
    "createdAtUtc": "2026-06-15T00:00:00Z",
    "permission": {
      "sessionId": "abc",
      "toolName": "Read",
      "toolUseId": "tool-1",
      "toolInput": { "file_path": "README.md" },
      "description": "Read README.md",
      "hookEventName": "PreToolUse"
    },
    "question": null
  }
]
```

`kind` 当前为 `permission` 或 `question`。客户端必须容忍未来新增值。

### `GET /api/pending/{actionId}`

成功返回 `PendingActionDto`。未知 action 返回 `404 ApiErrorDto`。

### `GET /api/pending/history`

返回最近 N 条已处理 pending action 的决定记录，供断线重连或后加入的展示端补看"已结束的审批到底是被谁、如何决定的"。

请求 query：

| 参数 | 类型 | 默认 | 说明 |
|------|------|------|------|
| `limit` | int | 100 | 返回最近多少条，最大 200（内部环形缓冲区上限）。 |

响应 `PendingHistoryDto`：

```json
{
  "entries": [
    {
      "actionId": "permission-....",
      "kind": "permission",
      "sessionId": "session-1",
      "source": "claude",
      "decision": "allow",
      "actor": "phone-A",
      "reason": null,
      "resolvedAtUtc": "2026-06-25T12:34:56Z"
    }
  ]
}
```

`decision` 取值：

| decision | 含义 |
|----------|------|
| `allow` | permission 被批准（本次） |
| `allow-always` | permission 被批准并要求 always allow |
| `deny` | permission 被拒绝，`reason` 附带原因 |
| `answered` | question 已作答 |
| `dismissed` | question 被取消 |
| `timeout` | 阻塞等待超时，CLI 侧会收到拒绝/取消响应 |

`actor` 是发起决策的展示端在 `POST` 时自报的标识，可为 `null`（兼容旧客户端；超时条目也记 `null`）。Runtime 不校验其唯一性，由展示端约定（如设备名 + 会话 ID）。多个展示端并发操作同一 action 时只有第一个成功；落后端会被 `404` 或 WS 的 `pending.resolved` 通知该 action 已被其他 actor 处理。

### `POST /api/permissions/{actionId}/allow`

请求 `PermissionDecisionRequest`：

```json
{
  "always": false,
  "actor": "phone-A"
}
```

`always=true` 表示用户希望将相同安全规则持久化为 always allow。是否支持取决于 source 能力。`actor` 可选，自报标识，会随 `pending.resolved` 广播给其他展示端。

成功响应：

```json
{
  "success": true
}
```

未知或已被其他展示端处理的 action 返回 `404`。多个展示端同时操作同一个 action 时，只有第一个成功；其他客户端应重新获取 `/api/pending`。

### `POST /api/permissions/{actionId}/deny`

请求 `PermissionDecisionRequest`：

```json
{
  "reason": "denied from mobile",
  "actor": "phone-A"
}
```

成功响应：

```json
{
  "success": true
}
```

### `POST /api/questions/{actionId}/answer`

适合一次性提交完整 answer map。

请求 `QuestionAnswerRequest`：

```json
{
  "answers": {
    "language": ["zh-CN"],
    "style": ["compact"]
  },
  "actor": "phone-A"
}
```

单答案 fallback：

```json
{
  "answer": "yes",
  "actor": "phone-A"
}
```

`actor` 可选，自报标识，会随 `pending.resolved` 广播。

成功响应：

```json
{
  "success": true
}
```

### `POST /api/questions/{actionId}/answer-current`

适合 HUD、手机、手表这类逐步显示当前问题的展示端。

请求 `QuestionCurrentAnswerRequest`：

```json
{
  "answers": ["选项 A"],
  "actor": "phone-A"
}
```

响应 `QuestionCurrentAnswerResultDto`：

```json
{
  "success": true,
  "resolved": false
}
```

`resolved=false` 表示 Runtime 已记录当前步骤答案并推进到下一题，客户端应重新获取 `/api/pending` 来渲染下一步。`resolved=true` 表示 hook response 已完成，pending action 会从后续快照中移除。

### `POST /api/questions/{actionId}/dismiss`

请求体为空。成功响应：

```json
{
  "success": true
}
```

Runtime 会向 CLI hook 返回关闭/拒绝类响应。

## DTO 字段表

### ApiHealthDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `status` | string | 当前固定为 `ok`。 |
| `startedAtUtc` | string | Runtime 启动时间，UTC。 |

### ApiVersionDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `product` | string | 产品名。 |
| `version` | string | Runtime assembly 版本。 |

### ApiCapabilitiesDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `hookInjection` | boolean | 是否支持 hook 注入。 |
| `approval` | boolean | 是否支持权限审批。 |
| `question` | boolean | 是否支持问答。 |
| `transcript` | boolean | 是否支持 transcript 消息读取。 |
| `realtime` | boolean | 是否支持实时事件。 |
| `realtimeProtocols` | string[] | 当前为 `["websocket"]`。 |
| `securityMode` | string | `localhost-token` 或 `remote-token`。 |

### SourceDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string | source key，例如 `codex`、`claude`。 |
| `displayName` | string | 展示名。 |
| `iconName` | string | 图标 key。 |
| `installed` | boolean | 当前 hook 是否已安装（Windows 侧）。 |
| `capabilities` | SourceCapabilitiesDto | source 能力。 |
| `sourceType` | string | `bundled` 或 `user`。 |

### SourceCapabilitiesDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `hookInstall` | boolean | 是否支持安装 hook。 |
| `approval` | boolean | 是否支持审批。 |
| `question` | boolean | 是否支持问答。 |
| `transcript` | boolean | 是否支持 transcript。 |
| `alwaysAllow` | boolean | 是否支持 always allow。 |

### SourceStatusDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `source` | string | source key。 |
| `supported` | boolean | 是否为已知 source。 |
| `installed` | boolean | hook 是否已安装；`probeOk=false` 时不可信。 |
| `displayName` | string | 展示名。 |
| `distro` | string? | WSL 状态时的发行版；Windows 侧省略。 |
| `probeOk` | boolean? | WSL 探测是否成功；Windows 侧省略。 |
| `error` | string? | 探测失败原因。 |

### SourceOperationResultDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `source` | string | source key。 |
| `success` | boolean | 操作是否成功。 |
| `installed` | boolean | 操作后是否仍处于已安装。 |
| `message` | string | 人类可读说明。 |
| `distro` | string? | WSL 操作实际使用的发行版。 |
| `code` | string? | 失败时的稳定错误码；成功时省略。 |

常见 `code`：

| code | 含义 |
| --- | --- |
| `unsupported_source` | 未知 source。 |
| `invalid_distro` | Docker/系统 distro 等不可用发行版。 |
| `missing_bridge` | 缺少 Windows bridge 可执行文件。 |
| `wsl_unavailable` | WSL 列表/探测/路径转换失败。 |
| `hook_write_failed` | 写入 hook 配置失败。 |
| `operation_failed` | 其他业务失败。 |

### WslDistroDto / WslDistrosDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `distros` | WslDistroDto[] | 用户侧发行版列表。 |
| `defaultDistro` | string? | 解析后的默认发行版。 |
| `name` | string | 发行版名。 |
| `state` | string | 如 `Running` / `Stopped`。 |
| `version` | number? | WSL 版本（通常 1/2）。 |
| `isDefault` | boolean | 是否为默认（过滤 Docker 后可能提升）。 |

### SessionDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `sessionId` | string | Runtime 会话 id。 |
| `source` | string | source key。 |
| `sourceDisplayName` | string | source 展示名。 |
| `projectName` | string? | 项目名。 |
| `workingDirectory` | string? | 工作目录。 |
| `status` | string | 会话状态。 |
| `currentToolName` | string? | 当前工具名。 |
| `currentToolDescription` | string? | 当前工具描述。 |
| `createdAtUtc` | string | 创建时间，UTC。 |
| `lastUpdatedAtUtc` | string | 最近更新时间，UTC。 |
| `trackedPid` | number? | 被跟踪进程 PID。 |
| `trackedProcessStartedAtUtc` | string? | 被跟踪进程启动时间，用于避免 PID 复用误判。 |
| `lastUserPrompt` | string? | 最近用户 prompt。 |
| `lastAssistantMessage` | string? | 最近 AI 回复。 |
| `completionText` | string? | 完成摘要。 |
| `transcriptPath` | string? | Runtime 内部 transcript 路径提示。展示端通常不应直接读取。 |
| `transcriptPosition` | number | Runtime 已读取的位置。 |
| `terminalApp` | string? | 终端类型提示。 |
| `terminalSessionId` | string? | 终端会话提示。 |
| `recentMessages` | ChatMessageDto[] | 最近消息。 |
| `toolHistory` | ToolHistoryEntryDto[] | 工具历史。 |

### PendingActionDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `actionId` | string | pending action id，所有操作都使用它。 |
| `kind` | string | `permission` 或 `question`。 |
| `sessionId` | string | 所属 session。 |
| `source` | string | source key。 |
| `sourceDisplayName` | string | source 展示名。 |
| `projectName` | string? | 项目名。 |
| `workingDirectory` | string? | 工作目录。 |
| `createdAtUtc` | string | 创建时间，UTC。 |
| `permission` | PermissionRequestDto? | 权限请求详情。 |
| `question` | QuestionDto? | 问题详情。 |

### PermissionRequestDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `sessionId` | string | 所属 session。 |
| `toolName` | string | 工具名。 |
| `toolUseId` | string? | 工具调用 id。 |
| `toolInput` | object? | 工具输入。客户端应容忍任意 JSON shape。 |
| `description` | string? | 展示描述。 |
| `hookEventName` | string | 原始/归一化 hook 事件名。 |

### QuestionDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `sessionId` | string | 所属 session。 |
| `id` | string? | 问题 id。 |
| `question` | string | 问题正文。 |
| `header` | string? | 可选标题。 |
| `options` | QuestionOptionDto[] | 当前问题选项。 |
| `multiSelect` | boolean | 是否多选。 |
| `isMultiQuestion` | boolean | 是否多步骤问题。 |
| `questions` | QuestionItemDto[] | 多步骤问题列表。 |
| `hookEventName` | string | hook 事件名。 |
| `isAskUserQuestion` | boolean | 是否 Claude AskUserQuestion。 |
| `isCodexRequestUserInput` | boolean | 是否 Codex request_user_input。 |
| `currentQuestionIndex` | number | 当前步骤 index。 |
| `currentAnswerKey` | string | 当前步骤答案 key。 |

### QuestionItemDto / QuestionOptionDto

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | string? | 问题项 id。仅 `QuestionItemDto`。 |
| `question` | string | 问题正文。仅 `QuestionItemDto`。 |
| `header` | string? | 标题。仅 `QuestionItemDto`。 |
| `options` | QuestionOptionDto[] | 选项。仅 `QuestionItemDto`。 |
| `multiSelect` | boolean | 是否多选。仅 `QuestionItemDto`。 |
| `allowFreeText` | boolean | 是否允许自由文本。仅 `QuestionItemDto`。 |
| `label` | string | 选项文案。仅 `QuestionOptionDto`。 |
| `description` | string? | 选项说明。仅 `QuestionOptionDto`。 |
| `value` | string? | 选项值。仅 `QuestionOptionDto`。 |

## WebSocket 事件

连接：

```text
ws://127.0.0.1:32145/api/events?token=<api_token>
```

消息格式 `HubEventDto`：

```json
{
  "type": "session.updated",
  "timestampUtc": "2026-06-15T00:00:00Z",
  "data": {}
}
```

已知事件：

| type | data | 建议行为 |
| --- | --- | --- |
| `session.updated` | `SessionDto[]` | 替换或 reconcile session 列表。 |
| `session.removed` | `{ "sessionId": string }` | 移除本地 session，或重新拉取 `/api/sessions`。 |
| `pending.updated` | `PendingActionDto[]` | 替换或 reconcile pending 列表。 |
| `pending.resolved` | `{ "actionId": string, "resolution": PendingResolutionDto, "pending": PendingActionDto[] }` | 移除已处理 action，reconcile pending 列表，并通过 `resolution` 得知它被谁（`actor`）、以何种方式（`decision`/`reason`）结束。 |
| `source.statusChanged` | `SourceOperationResultDto` 或 `SourceDto[]` | 重新拉取 `/api/sources`。 |

Runtime 支持多个 WebSocket client 同时连接。所有 client 会收到同一批事件。因为多个展示端可能同时操作同一个 pending action，展示端收到 `404` 或 `pending.resolved` 后应刷新 `/api/pending`，不要继续假设本地按钮仍有效。

## 前端推荐流程

1. 启动时请求 `GET /api/health`。
2. 带 token 请求 `GET /api/capabilities`。
3. 拉取 `GET /api/sessions`、`GET /api/pending`、必要时 `GET /api/sources`。
4. 连接 `WS /api/events`。
5. 收到已知事件后更新本地投影；不确定 payload 时直接重新拉取 REST 快照。
6. WebSocket 断线重连后，先重新拉取 REST 快照，再处理新事件。
7. 用户操作审批/问答时调用 action endpoint；成功或失败后都刷新 pending 快照。
8. UI-only 状态留在前端本地：选中项、动画、主题、声音、设备布局。

## Runtime 更新 manifest

`RuntimeHost` 和 `Bridge` 发布 ZIP 里包含本地 manifest：

```json
{
  "runtimeVersion": "1.0.1",
  "contractVersion": "1",
  "hostExe": "codeorbit-host.exe",
  "bridgeExe": "codeorbit-bridge.exe",
  "defaultPort": 32145
}
```

远程更新 manifest 供 HUD 下载 Runtime ZIP：

```json
{
  "runtimeVersion": "1.0.1",
  "contractVersion": "1",
  "downloadUrl": "https://.../CodeOrbit-Runtime-win-x64-v1.0.1.zip",
  "sha256": "..."
}
```

HUD 或其他前端应先校验 `contractVersion` 和 `sha256`，再推广新 Runtime payload。




