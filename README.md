# CodeOrbit

[简体中文](README_CN.md)

CodeOrbit is the centralized base process for CodeOrbit display clients. It ingests CLI hook events, normalizes sessions and pending approvals/questions, and exposes one token-authenticated REST/WebSocket contract to multiple displays.

This repository owns:

- `codeorbit-contracts`: public REST/WebSocket DTOs.
- `codeorbit-core`: hook models, source adapters, response builders, transcript readers, settings, and IPC protocol.
- `codeorbit-hub`: state management, hook server, source service, REST API, WebSocket fan-out, and local token store.
- `codeorbit-host`: standalone process.
- `codeorbit-bridge`: short-lived CLI hook bridge.
- Tests and docs.

The Windows HUD is a display client. It should consume this through `codeorbit-contracts` plus RuntimeHost/Bridge executable artifacts, not by compiling against internals. The HUD implementation lives in [CodeIsland-Windows](https://github.com/KelseySking/CodeIsland-Windows).

## Topology

Default local managed mode:

```text
Windows HUD -> starts codeorbit-host on 127.0.0.1 -> REST/WebSocket
CLI hook -> codeorbit-bridge -> named pipe (Windows) / Unix socket (Linux, macOS) -> state management
```

Shared remote mode is explicit. Bind with `--host 0.0.0.0` or `api_bind_host=0.0.0.0` only when the user intentionally wants LAN/mobile clients to connect with the API token.

## Build

```bash
cargo build --workspace
cargo test --workspace
```

Run a development instance:

```bash
cargo run -p codeorbit-host -- --token dev-token --port 32145 --no-repair
```

Then connect a display client to `http://127.0.0.1:32145` with token `dev-token`.

## Extensibility

CodeOrbit supports custom CLI sources through a **plugin system**. This allows you to add support for new AI CLI tools without recompiling.

### Plugin System Features

- **Automatic CLI Detection**: Plugins can define process names, environment variables, and path patterns to automatically detect which CLI is running
- **Hook Installation**: Plugins specify how to install hooks into the CLI's configuration files
- **Bundled Plugins**: Ships with built-in support for 19 CLI sources, including Claude Code, Codex CLI, Gemini CLI, Cursor, Kiro, Qwen Code, GitHub Copilot, and more
- **User Plugins**: Drop JSON files into the user sources directory (`%AppData%\CodeOrbit\sources\` on Windows, `~/.config/CodeOrbit/sources/` on Linux/macOS) to register custom CLIs

### Quick Start

Create a plugin file (e.g., `my-cli.json`) in the user sources directory (`%AppData%\CodeOrbit\sources\` on Windows, `~/.config/CodeOrbit/sources/` on Linux/macOS):

```json
{
  "schema_version": "2.0",
  "source": {
    "key": "my-cli",
    "display_name": "My CLI",
    "icon_name": "terminal",
    "permission_response_style": "claude-style"
  },
  "detection": {
    "process_names": ["my-cli"],
    "priority": 100
  },
  "hook_installation": {
    "format": "flat",
    "config_path": "~/.my-cli/hooks.json",
    "events": ["PreToolUse", "PostToolUse"],
    "timeout_seconds": 10
  }
}
```

Then install hooks via the REST API:

```bash
curl -X POST http://127.0.0.1:32145/api/sources/my-cli/install \
  -H "Authorization: Bearer <token>"
```

Or programmatically from Rust:

```rust
let installed = codeorbit_hub::config_installer::install_plugin("my-cli");
```

### Documentation

- **English**: [Plugin System Guide](docs/source-plugins.en.md) | [Plugin Schema Reference](docs/plugin-schema.en.md)
- **中文**: [插件系统指南](docs/source-plugins.md) | [插件 Schema 参考](docs/plugin-schema.md)

### Bundled Plugins

Ships with these built-in CLI plugins from `bundled-plugins/`:

| CLI | Source key / plugin file | Hook format | Events |
| --- | --- | --- | ---: |
| AntiGravity | `antigravity` / `antigravity.json` | `claude-matcher` | 12 |
| Claude Code | `claude` / `claude.json` | `claude-matcher` | 12 |
| Cline | `cline` / `cline.json` | `cline` | 5 |
| CodeBuddy | `codebuddy` / `codebuddy.json` | `claude-matcher` | 12 |
| Codex CLI | `codex` / `codex.json` | `codex` | 7 |
| Cursor | `cursor` / `cursor.json` | `flat` | 5 |
| Factory | `droid` / `droid.json` | `claude-matcher` | 12 |
| Gemini CLI | `gemini` / `gemini.json` | `nested` | 4 |
| GitHub Copilot | `copilot` / `copilot.json` | `copilot` | 7 |
| Hermes | `hermes` / `hermes.json` | `nested` | 6 |
| Kimi Code | `kimi` / `kimi.json` | `nested` | 6 |
| Kiro | `kiro` / `kiro.json` | `nested` | 6 |
| OpenCode | `opencode` / `opencode.json` | `nested` | 6 |
| Pi | `pi` / `pi.json` | `nested` | 6 |
| Qoder | `qoder` / `qoder.json` | `claude-matcher` | 12 |
| Qwen Code | `qwen` / `qwen.json` | `claude-matcher` | 12 |
| StepFun | `stepfun` / `stepfun.json` | `claude-matcher` | 12 |
| Trae | `trae` / `trae.json` | `flat` | 7 |
| WorkBuddy | `workbuddy` / `workbuddy.json` | `claude-matcher` | 12 |

## API And Display Clients

- [Documentation index](docs/README.md)
- [Full API reference](docs/api-reference.en.md)
- [Integration patterns for other apps](docs/integration-guide.en.md)
- [Ownership contract](docs/runtime-display-contract.md)
- [Display client quickstart](docs/external-display-client.en.md)

Chinese docs:

- [中文文档索引](docs/README_CN.md)
- [完整 API 文档](docs/api-reference.md)
- [其他应用集成方式](docs/integration-guide.md)
- [职责边界](docs/runtime-display-contract.zh-CN.md)
- [展示端快速开始](docs/external-display-client.md)

## Release Artifacts

Build optimized binaries with Cargo:

```bash
cargo build --release
```

This produces `codeorbit-host` and `codeorbit-bridge` (with a `.exe` suffix on Windows) under `target/release/`. Package them together with a `runtime-manifest.json` into a release ZIP. The Windows HUD can download update manifests and promote the ZIP payload into its local cache.

<center>This project has been shared on the [LINUX DO](https://linux.do).</center>
