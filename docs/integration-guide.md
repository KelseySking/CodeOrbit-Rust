# CodeOrbit Runtime Integration Guide

[English](integration-guide.en.md) | [文档索引](README_CN.md)

本文档说明其他应用如何集成 CodeOrbit Runtime。这里的“其他应用”包括 Windows/Mac/Linux 桌面应用、Web UI、浏览器插件、IDE 插件、手机 companion、硬件屏幕、企业内部面板，以及后续新的 CLI source adapter。

Runtime 的核心原则是：Runtime 做中心化状态和 CLI hook 控制面，其他应用只做展示、交互和设备体验。除非你是在开发 Runtime 自身或新增 source adapter，否则不要直接依赖 `codeorbit-core`、`codeorbit-hub`、Named Pipe、transcript 文件或 hook response builder。

## 集成方式总览

| 场景 | 推荐方式 | Runtime 生命周期 | 适用对象 |
| --- | --- | --- | --- |
| 官方桌面 HUD / 第三方桌面壳 | bundled managed Runtime | 应用启动时启动本地 Runtime，应用退出时关闭自己拥有的 localhost Runtime | WPF、WinUI、Avalonia、Electron、Tauri、Qt |
| 连接已存在 Runtime | external Runtime client | 不启动、不关闭 Runtime，只连接 API | 高级用户工具、调试面板、企业控制台 |
| 手机 / 平板 / 其他设备 | shared remote Runtime | Runtime 在电脑上常驻或由桌面端启动；移动端只连接 | iOS、Android、局域网 WebApp |
| Web 前端 | same-origin proxy 或桌面壳内嵌 | 推荐由本地壳或代理处理 token 和 CORS | React、Vue、Svelte、Next.js、浏览器面板 |
| IDE / 编辑器插件 | external Runtime client | 插件连接本机 Runtime，不直接处理 hook | VS Code、JetBrains、Cursor 插件 |
| 硬件屏幕 / IoT | remote display client | 只订阅状态和发送少量 action | 小屏幕、控制器、局域网设备 |
| 新 CLI source | Runtime source adapter | 由 Runtime 维护 hook 安装、事件解析和响应格式 | 新 AI CLI、内部 agent、自动化工具 |

## 方式一：桌面应用托管 Runtime

适合想提供“安装后即用”的桌面展示端。应用随包携带：

- `codeorbit-host.exe`
- `codeorbit-bridge.exe`
- `runtime-manifest.json`

启动流程：

1. 读取或生成本地 API token。
2. 选择端口，默认 `32145`。
3. 先请求 `GET /api/health`，如果已有健康 Runtime，直接连接。
4. 如果没有健康 Runtime，启动 `codeorbit-host.exe`：

```bash
codeorbit-host.exe --settings-dir "%APPDATA%\CodeOrbit" --host 127.0.0.1 --port 32145 --token <token> --owner-pid <app-pid> --shutdown-when-owner-exits
```

5. 等待 `/api/health` 成功。
6. 拉取 `/api/capabilities`、`/api/sessions`、`/api/pending`。
7. 建立 `WS /api/events?token=<token>`。
8. 应用退出时，只关闭自己启动且绑定在 localhost 的 Runtime。

注意：如果用户配置了 `--host 0.0.0.0` 或 `api_bind_host=0.0.0.0`，说明 Runtime 可能正在服务手机或其他展示端。桌面应用不应该在退出时杀掉这个 Runtime。

## 方式二：连接已存在 Runtime

适合调试工具、企业面板、IDE 插件或高级用户自定义前端。

启动流程：

1. 让用户输入 `baseUrl` 和 token，或从受信任位置读取。
2. 调用 `GET /api/health` 检查存活。
3. 调用 `GET /api/capabilities` 检查能力。
4. 连接 WebSocket 并按 API 文档处理事件。

这种方式不拥有 Runtime 生命周期。不要在应用退出时关闭 Runtime，也不要自动安装/卸载 hook，除非用户明确点击了对应按钮。

## 方式三：手机或其他设备远程连接

适合移动端 companion 或局域网硬件屏幕。

电脑端启动 Runtime 时必须显式开放监听：

```bash
codeorbit-host.exe --host 0.0.0.0 --port 32145 --token <strong-token>
```

移动端连接：

```text
http://<computer-lan-ip>:32145
ws://<computer-lan-ip>:32145/api/events?token=<strong-token>
```

安全要求：

- 默认不要自动开启 `0.0.0.0`。
- 不要把 token 打到日志里。
- 局域网二维码、pairing code、token rotation 和设备授权列表应作为单独安全任务实现。
- 公网暴露需要额外认证和 TLS 方案；当前 Runtime 默认不承担公网安全边界。

移动端 UI 建议：

- 只渲染关键 session、pending action 和 source 状态。
- 审批按钮调用 `/api/permissions/{actionId}/allow` 或 `/deny`。
- 问答使用 `/api/questions/{actionId}/answer-current`。
- 收到 `404` 时刷新 `/api/pending`，因为可能已有其他设备处理了该 action。

## 方式四：Web 前端集成

Web 前端有两种推荐形态。

桌面壳内嵌：Electron、Tauri、WebView2 等应用负责启动或连接 Runtime，Web UI 只调用壳提供的 API 或直连 localhost。token 保存在壳侧，不写入页面源码。

同源代理：本地服务或桌面壳提供同源 endpoint，例如 `/runtime-api/*` 转发到 `http://127.0.0.1:32145/api/*`，并在代理层附加 token。这样可以避免浏览器 CORS 和 token 泄露问题。

不推荐的方式：直接把 token 写进前端 bundle，或要求用户从任意网页直连本机 Runtime。浏览器 CORS、origin allowlist 和 pairing 需要单独设计后再开放。

## 方式五：插件或 IDE 面板

IDE 插件通常不应该自己处理 CLI hook。推荐流程：

1. 插件检测本机 Runtime 是否存在。
2. 如果用户选择“连接 CodeOrbit Runtime”，读取用户提供的 token 或通过未来 pairing 流程获取 token。
3. 插件通过 REST/WebSocket 展示状态。
4. 插件的审批/问答按钮调用 Runtime action endpoint。
5. 插件如果提供“安装 hook”按钮，也应该调用 `/api/sources/{source}/install`，而不是自己改写 Claude/Codex 配置。

## 方式六：硬件屏幕或低资源客户端

低资源客户端不需要完整状态机。推荐只做：

- 启动时拉取 `/api/sessions` 和 `/api/pending`。
- WebSocket 只识别 `session.updated`、`pending.updated`、`pending.resolved`。
- 未知事件直接触发 REST 快照刷新。
- 本地只保留最小投影，例如当前运行数量、最近 pending action、最新状态文字。

这样可以减少设备内存和 CPU 占用。

## 方式七：新增 CLI source

如果要让 Runtime 支持新的 AI CLI，不要从展示端接入。新的 source 应该进入 Runtime 仓库：

- 在 `crates/codeorbit-core/src/sources` 增加或扩展 source adapter。
- 在 Runtime/Core 中定义 source metadata、事件别名、能力和响应格式。
- 在 hook 安装/修复路径中加入该 source。
- 在 Bridge 或 source resolver 中识别该 CLI。
- 补充 Core/Bridge/Hub 测试。
- 更新 [API Reference 中文](api-reference.md) 和 [展示端合同](runtime-display-contract.zh-CN.md)。

展示端只会看到新的 `source`、`sourceDisplayName`、icon key、capabilities 和 session/pending DTO，不需要知道 source 内部 hook 格式。

## 最小集成清单

任何展示端至少需要实现：

1. 配置 `baseUrl` 和 token。
2. `GET /api/health` 存活检查。
3. `GET /api/capabilities` 能力检查。
4. `GET /api/sessions` 会话快照。
5. `GET /api/pending` 待处理快照。
6. `WS /api/events` 实时更新和断线重连。
7. 审批：`POST /api/permissions/{actionId}/allow|deny`。
8. 问答：`POST /api/questions/{actionId}/answer-current` 和 `/dismiss`。
9. 兼容未知字段、未知事件和 action 竞争处理。
10. 不依赖 Runtime 内部源码类型。

## Runtime 更新集成

如果你的应用负责携带 Runtime，可以实现自动更新：

1. 读取本地 `runtime-manifest.json`。
2. 请求远程 update manifest。
3. 比较 `runtimeVersion` 和 `contractVersion`。
4. 下载 Runtime ZIP。
5. 校验 `sha256`。
6. 解压到 staging 目录。
7. 检查 `codeorbit-host.exe` 和 `codeorbit-bridge.exe` 存在。
8. 停止自己拥有的本地私有 Runtime。
9. 原子切换 `runtime/current`。
10. 重启 Runtime 并验证 `/api/health`。

共享远程 Runtime 不应被任意展示端静默更新或关闭，除非用户明确选择了该操作。

## runtime-manifest.json 格式

Runtime ZIP 中的 `runtime-manifest.json` 包含以下字段：

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

**字段说明**：

- `runtimeVersion`：Runtime 版本号
- `contractVersion`：API 合约版本，用于判断展示端兼容性
- `hostExe`：RuntimeHost 可执行文件名
- `bridgeExe`：Bridge 可执行文件名
- `defaultPort`：默认 API 端口（可被 `--port` 或 `settings.json` 中的 `api_port` 覆盖）
- `defaultHost`：默认绑定地址（可被 `--host` 或 `settings.json` 中的 `api_bind_host` 覆盖）
- `defaultPipeName`：默认 Named Pipe 名称（可被 `--pipe-name` 覆盖）
- `defaultSettingsDir`：默认 settings 目录（可被 `--settings-dir` 覆盖）

**优先级顺序**：命令行参数 > SettingsManager > runtime-manifest.json > 硬编码默认值


