# CodeOrbit Runtime Integration Guide

[简体中文](integration-guide.md) | [Documentation index](README.md)

This guide explains how other applications integrate CodeOrbit Runtime. Applications include desktop apps, web UIs, browser extensions, IDE plugins, mobile companions, hardware displays, internal dashboards, and future CLI source adapters.

Runtime is the centralized state and CLI hook control plane. Other applications should provide presentation, interaction, and device-specific UX. Unless you are developing Runtime itself or adding a CLI source adapter, do not depend on `codeorbit-core`, `codeorbit-hub`, Named Pipe internals, transcript files, or hook response builders.

## Integration Mode Overview

| Scenario | Recommended mode | Runtime lifecycle | Examples |
| --- | --- | --- | --- |
| Official desktop HUD / third-party desktop shell | Bundled managed Runtime | App starts local Runtime and stops only the localhost Runtime it owns | WPF, WinUI, Avalonia, Electron, Tauri, Qt |
| Connect to an existing Runtime | External Runtime client | App does not start or stop Runtime | Debug tools, enterprise dashboard, advanced user tools |
| Phone / tablet / another device | Shared remote Runtime | Runtime runs on the computer; mobile device only connects | iOS, Android, LAN web app |
| Web frontend | Same-origin proxy or desktop shell | A local shell/proxy handles token and CORS | React, Vue, Svelte, Next.js, browser panel |
| IDE/editor plugin | External Runtime client | Plugin connects to local Runtime and does not handle hooks | VS Code, JetBrains, Cursor plugins |
| Hardware display / IoT | Remote display client | Subscribe to state and send small action requests | Small screen, controller, LAN device |
| New CLI source | Runtime source adapter | Runtime owns hook install, event parsing, response format | New AI CLI, internal agent, automation tool |

## Mode 1: Desktop App Bundles Runtime

Use this when you want an install-and-run desktop display. Bundle `codeorbit-host.exe`, `codeorbit-bridge.exe`, and `runtime-manifest.json`.

Startup flow:

1. Read or generate a local API token.
2. Select a port; default is `32145`.
3. Probe `GET /api/health`; if a healthy Runtime already exists, connect to it.
4. If no healthy Runtime exists, start RuntimeHost:

```bash
codeorbit-host.exe --settings-dir "%APPDATA%\CodeOrbit" --host 127.0.0.1 --port 32145 --token <token> --owner-pid <app-pid> --shutdown-when-owner-exits
```

5. Wait for `/api/health`.
6. Fetch `/api/capabilities`, `/api/sessions`, and `/api/pending`.
7. Connect `WS /api/events?token=<token>`.
8. On app exit, stop only the Runtime process that this app owns and only when it is bound to localhost.

If the user configured `--host 0.0.0.0` or `api_bind_host=0.0.0.0`, Runtime may be serving phones or other displays. Do not kill that Runtime when a desktop display exits.

## Mode 2: Connect To Existing Runtime

Use this for debug tools, enterprise panels, IDE plugins, or advanced custom frontends. Ask the user for `baseUrl` and token, probe `/api/health`, read `/api/capabilities`, then subscribe to WebSocket events. This mode does not own Runtime lifecycle.

## Mode 3: Mobile Or Remote Device

The computer must explicitly expose Runtime:

```bash
codeorbit-host.exe --host 0.0.0.0 --port 32145 --token <strong-token>
```

Device connection:

```text
http://<computer-lan-ip>:32145
ws://<computer-lan-ip>:32145/api/events?token=<strong-token>
```

Do not enable `0.0.0.0` automatically, do not log tokens, and implement pairing/token rotation/device allowlists as a separate security task.

## Mode 4: Web Frontend

Recommended shapes:

- Desktop shell embedding: Electron, Tauri, WebView2, or another shell starts/connects Runtime. Keep the token in the shell layer.
- Same-origin proxy: a local service exposes `/runtime-api/*`, forwards to `http://127.0.0.1:32145/api/*`, and attaches the token at the proxy layer.

Avoid putting the token into a frontend bundle or asking arbitrary web pages to connect directly to local Runtime.

## Mode 5: Plugin Or IDE Panel

IDE plugins should not implement CLI hook handling. Detect or pair with local Runtime, display state through REST/WebSocket, and use Runtime action endpoints for approval/question buttons. If exposing hook install buttons, call `/api/sources/{source}/install`; do not rewrite Claude/Codex config directly.

## Mode 6: Hardware Or Low-Resource Client

Fetch `/api/sessions` and `/api/pending` on startup, recognize only the event types you need, refetch REST snapshots for unknown events, and keep the smallest possible local projection.

## Mode 7: Add A New CLI Source

New AI CLI support belongs in Runtime, not in display clients. Add or extend a source adapter in `crates/codeorbit-core/src/sources`, update hook install/repair and Bridge/source resolver behavior, add tests, then update API and display contract docs.

## Minimum Display Checklist

1. Configure `baseUrl` and token.
2. `GET /api/health`.
3. `GET /api/capabilities`.
4. `GET /api/sessions`.
5. `GET /api/pending`.
6. `WS /api/events` with reconnect.
7. Approval: `/api/permissions/{actionId}/allow|deny`.
8. Question: `/api/questions/{actionId}/answer-current` and `/dismiss`.
9. Unknown field/event/action-race tolerance.
10. No dependency on Runtime internal source types.

## Runtime Update Integration

If your app carries Runtime artifacts: read local `runtime-manifest.json`, fetch remote update manifest, compare `runtimeVersion` and `contractVersion`, download Runtime ZIP, verify `sha256`, extract to staging, check RuntimeHost/Bridge executables, stop only your owned local private Runtime, atomically switch `runtime/current`, restart Runtime, and verify `/api/health`.

Do not silently update or close a shared remote Runtime unless the user explicitly requests it.

## runtime-manifest.json Format

The `runtime-manifest.json` file in Runtime ZIP contains:

```json
{
  "runtimeVersion": "1.0.1",
  "contractVersion": "1",
  "hostExe": "codeorbit-host.exe",
  "bridgeExe": "codeorbit-bridge.exe",
  "defaultPort": 32145,
  "defaultHost": "127.0.0.1",
  "defaultPipeName": null,
  "defaultSettingsDir": null
}
```

**Field descriptions**:

- `runtimeVersion`: Runtime version number
- `contractVersion`: API contract version for display compatibility check
- `hostExe`: RuntimeHost executable name
- `bridgeExe`: Bridge executable name
- `defaultPort`: Default API port (overridable by `--port` or `api_port` in settings.json)
- `defaultHost`: Default bind address (overridable by `--host` or `api_bind_host` in settings.json)
- `defaultPipeName`: Default Named Pipe name (overridable by `--pipe-name`)
- `defaultSettingsDir`: Default settings directory (overridable by `--settings-dir`)

**Priority order**: Command-line args > SettingsManager > runtime-manifest.json > Hard-coded defaults

