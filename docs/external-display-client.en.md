# External Display Client Quickstart

[简体中文](external-display-client.md) | [Documentation index](README.md)

This document is for developers building CodeOrbit displays outside the WPF HUD, such as web UIs, plugins, terminal panels, hardware bridges, or mobile companions.

The stable boundary is the Runtime API described in [Display Contract](runtime-display-contract.md). Full endpoints, DTOs, and WebSocket events are documented in [API Reference](api-reference.en.md). Desktop, web, mobile, plugin, and hardware integration modes are documented in [Integration Guide](integration-guide.en.md).

Display clients should depend on REST DTOs, WebSocket events, and operation endpoints. They should not depend on WPF view models, Hub concrete classes, Named Pipe hook handling, transcript files, or source-specific response builders.

## Runtime Connection

Runtime listens locally by default:

```text
http://127.0.0.1:32145
```

The WPF HUD starts `codeorbit-host` in managed mode and writes the local API token to `%APPDATA%\CodeOrbit\settings.json` as `api_token`.

For standalone Runtime development, pass a token explicitly:

```bash
cargo run -p codeorbit-host -- --token dev-token --port 32145
```

Then use the same token in your display client.

## Auth

`GET /api/health` is unauthenticated. Every other endpoint requires the Runtime token.

REST clients should prefer headers:

```http
Authorization: Bearer <api_token>
```

WebSocket clients may pass the token in query string:

```text
ws://127.0.0.1:32145/api/events?token=<api_token>
```

Do not log tokens. Do not persist tokens outside trusted local settings. Do not expose the API to LAN/public addresses without a dedicated pairing/security design.

## Minimum Client Flow

1. Probe Runtime with `GET /api/health`.
2. Read feature flags with `GET /api/capabilities`.
3. Fetch initial snapshots: `/api/sessions`, `/api/pending`, and `/api/sources` if the UI shows source status.
4. Connect `WS /api/events`.
5. After WebSocket reconnect, refetch snapshots before processing new events.
6. Log and tolerate unknown event types. For known `session.*`, `pending.*`, or `source.*` families, refetch the matching REST snapshot if the client does not fully understand the payload.

## Pending Operations

The `actionId` comes from `GET /api/pending`.

Allow permission:

```http
POST /api/permissions/{actionId}/allow
Content-Type: application/json

{ "always": false }
```

Deny permission:

```http
POST /api/permissions/{actionId}/deny
Content-Type: application/json

{ "reason": "denied from external display" }
```

Answer the current visible question step:

```http
POST /api/questions/{actionId}/answer-current
Content-Type: application/json

{ "answers": ["selected-value-or-free-text"] }
```

When `resolved=false`, keep the pending action and refetch `/api/pending`; Runtime has advanced to the next question step. When `resolved=true`, the next pending snapshot removes the action.

Use `POST /api/questions/{actionId}/answer` only when the client owns a full keyed answer map. HUD-style step-by-step displays should use `answer-current`.

## Reconnect And Compatibility

Treat Runtime state as server state. Refetch `/api/sessions` and `/api/pending` on startup and WebSocket reconnect, tolerate unknown status/source/DTO/event values, keep UI-only state locally, and extend the public contract if more data is needed.

