# CodeOrbit

[English](README.md)

CodeOrbit 是一个中心化基座。它负责接入本机 CLI hook 事件，归一化会话、审批和问答状态，并通过带 token 认证的 REST/WebSocket 接口把同一份状态提供给多个展示端。

这个仓库负责：

- `codeorbit-contracts`：公开 REST/WebSocket DTO 合同。
- `codeorbit-core`：hook 模型、source adapter、响应构造、transcript 读取、settings、IPC 协议。
- `codeorbit-hub`：Runtime 状态、HookServer、source service、REST API、WebSocket 广播、本地 token 存储。
- `codeorbit-host`：独立 Runtime 进程。
- `codeorbit-bridge`：短生命周期 CLI hook bridge。
- Runtime 侧测试与文档。

Windows HUD 只是官方展示客户端。它应该通过 `codeorbit-contracts` 和 RuntimeHost/Bridge 可执行产物集成，不应该继续编译依赖内部实现。HUD 实现仓库位于 [CodeIsland-Windows](https://github.com/KelseySking/CodeIsland-Windows)。

## 拓扑

默认本地 managed 模式：

```text
Windows HUD -> 启动 127.0.0.1 上的 codeorbit-host -> REST/WebSocket
CLI hook -> codeorbit-bridge -> named pipe（Windows）/ Unix socket（Linux、macOS）-> 状态管理
```

共享远程模式必须显式开启。只有当用户明确希望手机、Web、硬件屏幕或其他设备通过局域网连接时，才使用 `--host 0.0.0.0` 或 `api_bind_host=0.0.0.0`。默认不要开放局域网监听。

## 构建

```bash
cargo build --workspace
cargo test --workspace
```

开发时启动：

```bash
cargo run -p codeorbit-host -- --token dev-token --port 32145 --no-repair
```

展示端连接 `http://127.0.0.1:32145`，token 使用 `dev-token`。

## 可扩展性

CodeOrbit 支持通过**插件系统**扩展 CLI 源。这使你可以添加新的 AI CLI 工具支持，无需重新编译。

### 插件系统功能

- **自动 CLI 检测**：插件可以定义进程名、环境变量、路径模式来自动检测正在运行的 CLI
- **Hook 安装**：插件指定如何将 hook 安装到 CLI 的配置文件中
- **内置插件**：自带 19 个 CLI 源，包括 Claude Code、Codex CLI、Gemini CLI、Cursor、Kiro、Qwen Code、GitHub Copilot 等
- **用户插件**：将 JSON 文件放入用户源目录（Windows 为 `%AppData%\CodeOrbit\sources\`，Linux/macOS 为 `~/.config/CodeOrbit/sources/`）即可注册自定义 CLI

### 快速开始

在用户源目录（Windows 为 `%AppData%\CodeOrbit\sources\`，Linux/macOS 为 `~/.config/CodeOrbit/sources/`）中创建插件文件（例如 `my-cli.json`）：

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

然后通过 REST API 安装 hook：

```bash
curl -X POST http://127.0.0.1:32145/api/sources/my-cli/install \
  -H "Authorization: Bearer <token>"
```

或在 Rust 中以编程方式调用：

```rust
let installed = codeorbit_hub::config_installer::install_plugin("my-cli");
```

### 文档

- **中文**：[插件系统指南](docs/source-plugins.md) | [插件 Schema 参考](docs/plugin-schema.md)
- **English**: [Plugin System Guide](docs/source-plugins.en.md) | [Plugin Schema Reference](docs/plugin-schema.en.md)

### 内置插件

从 `bundled-plugins/` 自带以下内置 CLI 插件：

| CLI | Source key / 插件文件 | Hook 格式 | 事件数 |
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

## 接口和展示端开发

- [中文文档索引](docs/README_CN.md)
- [完整 API 文档](docs/api-reference.md)
- [其他应用集成方式](docs/integration-guide.md)
- [职责边界](docs/runtime-display-contract.zh-CN.md)
- [展示端快速开始](docs/external-display-client.md)

英文文档：

- [Documentation index](docs/README.md)
- [Full API reference](docs/api-reference.en.md)
- [Integration patterns for other apps](docs/integration-guide.en.md)
- [Ownership contract](docs/runtime-display-contract.md)
- [Display client quickstart](docs/external-display-client.en.md)

## 发布产物

`cargo build --release` 只生成优化编译产物，不是最终发版包。最终压缩包用发版脚本生成：

```bash
python scripts/package-release.py --clean
```

脚本从根 `Cargo.toml` 读取唯一版本号，构建 release 二进制，只暂存 `codeorbit-host`、`codeorbit-bridge`、`bundled-plugins/`、`runtime-manifest.json` 和 `LICENSE`，并在 `release/` 下按 target 分别生成压缩包。

重复传入 `--target` 可以生成多个目标包：

```bash
python scripts/package-release.py --target x86_64-pc-windows-msvc --target x86_64-unknown-linux-gnu
```

脚本不负责安装 Rust target 或交叉链接工具链；跨平台打包前需要先准备对应构建环境。

## 前端集成建议

前端展示端只负责 UI、交互、动画、主题和设备适配。它应该：

- 启动时读取 Runtime `/api/health`、`/api/capabilities`、`/api/sessions`、`/api/pending`。
- 通过 `WS /api/events` 订阅变化，断线重连后重新拉取 REST 快照。
- 对审批、拒绝、问答、关闭等用户操作调用 REST action endpoint。
- 保持 UI-only 状态在本地，例如选中项、窗口位置、主题、动画、声音。
- 不读取 Hub/Core/Bridge 内部类型，不直接读 transcript 文件，不自己实现 hook response。

官方 Windows HUD 的默认体验是：启动 HUD 时启动本地进程，退出 HUD 时只关闭自己拥有的本地私有进程；如果显式绑定到 `0.0.0.0` 进入共享远程模式，HUD 退出时不关闭进程，避免断开手机或其他展示端。

<center>该项目已在 [LINUX DO](https://linux.do) 社区分享。</center>
