# CodeOrbit Runtime API Reference

[简体中文](api-reference.md) | [Documentation index](README.md)

This document is for Windows HUD, web, mobile, hardware displays, and third-party display clients. It describes the public REST/WebSocket contract exposed by Runtime. Source definitions live in `src/codeorbit-contracts/ApiDtos.cs` and `src/codeorbit-hub/CodeOrbitApiHost.cs`.

## Base URL And Auth

Default base URL:

```text
http://127.0.0.1:32145
```

Every REST endpoint is under `/api`. `GET /api/health` is unauthenticated; all other endpoints require the Runtime token.

Accepted token carriers:

```http
Authorization: Bearer <api_token>
X-CodeOrbit-Token: <api_token>
```

WebSocket clients may pass the token in the query string:

```text
ws://127.0.0.1:32145/api/events?token=<api_token>
```

Runtime is localhost-only by default. Remote access must be explicitly enabled:

```bash
codeorbit-host.exe --host 0.0.0.0 --port 32145 --token <token>
```

Browser CORS, pairing codes, token rotation, and public-internet exposure are not default Runtime features. Web displays should use a same-origin proxy, desktop shell, or a later pairing/security design.

## RuntimeHost Arguments

| Argument | Description |
| --- | --- |
| `--settings-dir <path>` | Directory containing `settings.json`. Defaults to `%APPDATA%\CodeOrbit`. |
| `--port <port>` | REST/WebSocket port. Normalized to `1024..65535`. |
| `--host <host>` | Bind host. Defaults to `127.0.0.1`; `0.0.0.0` means explicit shared remote mode. |
| `--token <token>` | API token. If omitted, Runtime reads or creates one in settings. |
| `--pipe-name <name>` | Named Pipe name, mostly for tests or isolated instances. |
| `--owner-pid <pid>` | Frontend process that owns this Runtime instance. |
| `--shutdown-when-owner-exits` | Exit Runtime when the owner exits. Use only for local private managed mode. |
| `--no-repair` | Do not repair installed source hooks on startup. |

Only one RuntimeHost instance may bind a given API port. A second host for the same port exits with an error.

## Response Rules

- Successful responses are JSON.
- Date/time fields are UTC `DateTimeOffset` values serialized as ISO 8601 strings.
- Missing resources usually return `404 ApiErrorDto`.
- Unauthorized requests return `401 ApiErrorDto`.
- Display clients must tolerate unknown fields, unknown session statuses, unknown source ids, and unknown WebSocket event types.

Error shape:

```json
{
  "code": "unauthorized",
  "message": "Missing or invalid CodeOrbit API token"
}
```

## Integration Modes

If you are building a desktop app, web UI, browser extension, IDE plugin, mobile companion, hardware display, or internal dashboard, read [Integration Guide](integration-guide.en.md). The API rule is the same for every display: use REST/WebSocket only. New CLI sources belong inside Runtime/Core/Bridge, not in display clients.

## Endpoint Overview

### Runtime

| Method | Path | Auth | Description |
| --- | --- | --- | --- |
| `GET` | `/api/health` | No | Runtime liveness. |
| `GET` | `/api/version` | Yes | Runtime product and version. |
| `GET` | `/api/capabilities` | Yes | Feature flags and security mode. |

### Sources And Runtime Assets

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/api/sources` | List supported CLI sources and install status. |
| `GET` | `/api/sources/{source}` | Get source status. |
| `GET` | `/api/sources/{source}/status` | Source status alias. |
| `GET` | `/api/sources/wsl/distros` | List installed WSL distributions. |
| `GET` | `/api/sources/{source}/wsl/status?distro=<name>` | Get source hook status inside a WSL distribution. |
| `POST` | `/api/sources/{source}/install` | Install or update the CodeOrbit hook for one source. |
| `POST` | `/api/sources/{source}/uninstall` | Remove CodeOrbit-owned hook entries. |
| `POST` | `/api/sources/{source}/repair` | Repair one source hook configuration. |
| `POST` | `/api/sources/{source}/wsl/install?distro=<name>` | Install a source hook inside WSL that calls the Windows bridge. |
| `POST` | `/api/sources/{source}/wsl/uninstall?distro=<name>` | Remove CodeOrbit-owned hook entries inside WSL. |
| `POST` | `/api/sources/{source}/wsl/repair?distro=<name>` | Repair one WSL source hook configuration. |
| `POST` | `/api/sources/repair-all` | Repair every installed source. |
| `GET` | `/api/runtime-assets` | Get Runtime hook script and bridge paths. |
| `POST` | `/api/runtime-assets/repair` | Repair shared Runtime assets. |

### Sessions

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/api/sessions` | Current session snapshot list. |
| `GET` | `/api/sessions/{sessionId}` | One session. |
| `GET` | `/api/sessions/{sessionId}/messages` | Recent messages for one session. |
| `POST` | `/api/sessions/{sessionId}/dismiss` | Remove a Runtime session and resolve related pending hooks. |
| `POST` | `/api/sessions/{sessionId}/activate-terminal` | Request terminal activation for a session. |

### Pending Actions

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/api/pending` | Pending approval/question list. |
| `GET` | `/api/pending/{actionId}` | One pending action. |
| `POST` | `/api/permissions/{actionId}/allow` | Allow a permission request. |
| `POST` | `/api/permissions/{actionId}/deny` | Deny a permission request. |
| `POST` | `/api/questions/{actionId}/answer` | Submit a full keyed answer map. |
| `POST` | `/api/questions/{actionId}/answer-current` | Answer the current visible question step. |
| `POST` | `/api/questions/{actionId}/dismiss` | Dismiss a question. |

### Realtime

| Method | Path | Description |
| --- | --- | --- |
| `WS` | `/api/events?token=<api_token>` | WebSocket event stream; multiple display clients may connect simultaneously. |

## Endpoint Details

### `GET /api/health`

No token required.

```json
{
  "status": "ok",
  "startedAtUtc": "2026-06-15T00:00:00Z"
}
```

### `GET /api/version`

```json
{
  "product": "CodeOrbit Runtime",
  "version": "1.0.1.0"
}
```

### `GET /api/capabilities`

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

`securityMode` is `localhost-token` for loopback binding and `remote-token` for non-loopback binding.

### `GET /api/sources`

Returns `SourceDto[]`:

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

### Source status and operations

`GET /api/sources/{source}` and `GET /api/sources/{source}/status` return `SourceStatusDto`:

```json
{
  "source": "codex",
  "supported": true,
  "installed": true,
  "displayName": "Codex"
}
```

`POST /api/sources/{source}/install`, `/uninstall`, and `/repair` have no request body. They return `SourceOperationResultDto`:

```json
{
  "source": "codex",
  "success": true,
  "installed": true,
  "message": "installed"
}
```

Failures usually return `400` with `success=false`.

### WSL source operations

`GET /api/sources/wsl/distros` returns installed WSL distributions:

```json
{
  "distros": ["Ubuntu"]
}
```

`GET /api/sources/{source}/wsl/status?distro=Ubuntu` returns `SourceStatusDto`.

`POST /api/sources/{source}/wsl/install`, `/uninstall`, and `/repair` take an optional `distro` query. If omitted, Runtime uses the default WSL distro. WSL hooks call the Windows `codeorbit-bridge.exe` through WSL interop with explicit `--source <source>`.

### Runtime assets

`GET /api/runtime-assets` returns:

```json
{
  "runtimeDirectory": "C:\\Users\\name\\.CodeOrbit",
  "hookScriptPath": "C:\\Users\\name\\.CodeOrbit\\CodeOrbit-hook.ps1",
  "bridgeExePath": "C:\\Users\\name\\.CodeOrbit\\CodeOrbit-bridge.exe",
  "installed": true
}
```

`POST /api/runtime-assets/repair` returns `{ "success": true, "assets": RuntimeAssetsDto }`.

### Sessions

`GET /api/sessions` returns `SessionDto[]`. Refetch it after startup, WebSocket reconnect, or unknown `session.*` events.

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
    "lastUserPrompt": "continue task",
    "lastAssistantMessage": "checking project",
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

Known statuses include `Idle`, `Processing`, `Running`, `WaitingQuestion`, `WaitingApproval`, `Completed`, and `Error`. Clients must tolerate new values.
### Session detail, messages, dismiss, activate

- `GET /api/sessions/{sessionId}` returns `SessionDto` or `404`.
- `GET /api/sessions/{sessionId}/messages` returns `ChatMessageDto[]`.
- `POST /api/sessions/{sessionId}/dismiss` returns `{ "success": true }` or `404`. This is a Runtime-level removal; simple UI hiding should stay local to the display.
- `POST /api/sessions/{sessionId}/activate-terminal` returns `{ "success": true }` or `404`.

### `GET /api/pending`

Returns `PendingActionDto[]`:

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

`kind` is currently `permission` or `question`; clients must tolerate new values.

### Resolution history

`GET /api/pending/history?limit=100` returns the most recent resolved pending actions as `PendingHistoryDto`, so late-joining or reconnecting displays can see not just that an action ended, but *how* and *by whom* it was decided:

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

| decision | meaning |
| --- | --- |
| `allow` | permission approved (this time) |
| `allow-always` | permission approved with "always allow" requested |
| `deny` | permission denied (`reason` carries the cause) |
| `answered` | question answered |
| `dismissed` | question dismissed |
| `timeout` | blocking hook wait timed out; CLI receives a deny/dismiss response |

`actor` is the self-reported identifier the display passed in the resolving `POST`. It may be `null` for older clients and for timeout entries (which have no actor). Runtime does not enforce uniqueness; displays agree on a convention (e.g. device name + session id).

### Permission actions

Allow request body:

```json
{
  "always": false,
  "actor": "phone-A"
}
```

Deny request body:

```json
{
  "reason": "denied from mobile",
  "actor": "phone-A"
}
```

`actor` is optional and self-reported; it is broadcast to other displays via `pending.resolved`. Both return `{ "success": true }` or `404`. If multiple displays act on the same `actionId`, only the first succeeds; others should refetch `/api/pending`.

### Question actions

Full answer map:

```json
{
  "answers": {
    "language": ["en-US"],
    "style": ["compact"]
  },
  "actor": "phone-A"
}
```

Single-answer fallback:

```json
{
  "answer": "yes",
  "actor": "phone-A"
}
```

Step-by-step current question answer:

```json
{
  "answers": ["Option A"],
  "actor": "phone-A"
}
```

`POST /api/questions/{actionId}/answer-current` returns:

```json
{
  "success": true,
  "resolved": false
}
```

`resolved=false` means Runtime advanced to the next question step; refetch `/api/pending`. `resolved=true` means the hook response is complete and the pending action will disappear.

`POST /api/questions/{actionId}/dismiss` has no request body and returns `{ "success": true }` or `404`.

## DTO Field Reference

### Runtime DTOs

| DTO | Fields |
| --- | --- |
| `ApiHealthDto` | `status`, `startedAtUtc` |
| `ApiVersionDto` | `product`, `version` |
| `ApiCapabilitiesDto` | `hookInjection`, `approval`, `question`, `transcript`, `realtime`, `realtimeProtocols`, `securityMode` |
| `ApiErrorDto` | `code`, `message` |
| `RuntimeAssetsDto` | `runtimeDirectory`, `hookScriptPath`, `bridgeExePath`, `installed` |

### Source DTOs

| DTO | Fields |
| --- | --- |
| `SourceDto` | `id`, `displayName`, `iconName`, `installed`, `capabilities` |
| `SourceCapabilitiesDto` | `hookInstall`, `approval`, `question`, `transcript`, `alwaysAllow` |
| `SourceStatusDto` | `source`, `supported`, `installed`, `displayName` |
| `SourceOperationResultDto` | `source`, `success`, `installed`, `message` |

### Session DTOs

| DTO | Fields |
| --- | --- |
| `SessionDto` | `sessionId`, `source`, `sourceDisplayName`, `projectName`, `workingDirectory`, `status`, `currentToolName`, `currentToolDescription`, `createdAtUtc`, `lastUpdatedAtUtc`, `trackedPid`, `trackedProcessStartedAtUtc`, `lastUserPrompt`, `lastAssistantMessage`, `completionText`, `transcriptPath`, `transcriptPosition`, `terminalApp`, `terminalSessionId`, `recentMessages`, `toolHistory` |
| `ChatMessageDto` | `isUser`, `text`, `timestampUtc` |
| `ToolHistoryEntryDto` | `toolName`, `timestampUtc`, `description`, `success` |

### Pending DTOs

| DTO | Fields |
| --- | --- |
| `PendingActionDto` | `actionId`, `kind`, `sessionId`, `source`, `sourceDisplayName`, `projectName`, `workingDirectory`, `createdAtUtc`, `permission`, `question` |
| `PendingResolutionDto` | `actionId`, `kind`, `sessionId`, `source`, `decision`, `actor`, `reason`, `resolvedAtUtc` |
| `PendingHistoryDto` | `entries` (list of `PendingResolutionDto`) |
| `PermissionRequestDto` | `sessionId`, `toolName`, `toolUseId`, `toolInput`, `description`, `hookEventName` |
| `PermissionDecisionRequest` | `always`, `reason`, `actor` |
| `QuestionDto` | `sessionId`, `id`, `question`, `header`, `options`, `multiSelect`, `isMultiQuestion`, `questions`, `hookEventName`, `isAskUserQuestion`, `isCodexRequestUserInput`, `currentQuestionIndex`, `currentAnswerKey` |
| `QuestionItemDto` | `id`, `question`, `header`, `options`, `multiSelect`, `allowFreeText` |
| `QuestionOptionDto` | `label`, `description`, `value` |
| `QuestionAnswerRequest` | `answer`, `answers`, `actor` |
| `QuestionCurrentAnswerRequest` | `answers`, `actor` |
| `QuestionCurrentAnswerResultDto` | `success`, `resolved` |

## WebSocket Events

Connect to:

```text
ws://127.0.0.1:32145/api/events?token=<api_token>
```

Message shape:

```json
{
  "type": "session.updated",
  "timestampUtc": "2026-06-15T00:00:00Z",
  "data": {}
}
```

Known events:

| Type | Data | Recommended client behavior |
| --- | --- | --- |
| `session.updated` | `SessionDto[]` | Replace or reconcile session list. |
| `session.removed` | `{ "sessionId": string }` | Remove the session locally or refetch `/api/sessions`. |
| `pending.updated` | `PendingActionDto[]` | Replace or reconcile pending list. |
| `pending.resolved` | `{ "actionId": string, "resolution": PendingResolutionDto, "pending": PendingActionDto[] }` | Remove resolved action, reconcile pending list, and learn via `resolution` who (`actor`) ended it and how (`decision`/`reason`). |
| `source.statusChanged` | `SourceOperationResultDto` or `SourceDto[]` | Refetch `/api/sources`. |

Runtime broadcasts events to every connected display client. Use REST snapshots as recovery state after reconnect.

## Recommended Display Flow

1. `GET /api/health`.
2. `GET /api/capabilities` with token.
3. Fetch `/api/sessions`, `/api/pending`, and optionally `/api/sources`.
4. Connect `WS /api/events`.
5. Reconcile known events; refetch snapshots for unknown payloads.
6. After WebSocket reconnect, refetch snapshots before processing new events.
7. After any approval/question action, refetch pending state whether the call succeeds or fails.
8. Keep UI-only state local: selection, animation, theme, sound, and device layout.

## Runtime Update Manifests

Runtime ZIP payload manifest:

```json
{
  "runtimeVersion": "1.0.1",
  "contractVersion": "1",
  "hostExe": "codeorbit-host.exe",
  "bridgeExe": "codeorbit-bridge.exe",
  "defaultPort": 32145
}
```

Remote update manifest:

```json
{
  "runtimeVersion": "1.0.1",
  "contractVersion": "1",
  "downloadUrl": "https://.../CodeOrbit-Runtime-win-x64-v1.0.1.zip",
  "sha256": "..."
}
```

Frontend apps should validate `contractVersion` and `sha256` before promoting a new Runtime payload.
