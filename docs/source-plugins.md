# CLI 源插件系统

CodeOrbit Runtime 支持通过 JSON 插件文件扩展 CLI 源，无需重新编译。插件系统支持自动 CLI 检测和 hook 安装。

## 概述

插件系统提供三大核心能力：

1. **源定义**：定义 CLI 标识和显示属性
2. **自动检测**：基于进程名、环境变量和路径自动检测正在运行的 CLI
3. **Hook 安装**：自动将 hook 安装到 CLI 的配置文件中

## 快速开始

### 基础插件（Schema 2.0）

在 `%AppData%\CodeOrbit\sources\` 中创建 JSON 文件（例如 `my-cli.json`）：

```json
{
  "schema_version": "2.0",
  "source": {
    "key": "my-cli",
    "display_name": "My AI CLI",
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

### 安装 Hook

使用 `ConfigInstaller` 自动安装 hook：

```csharp
using codeorbit-core.Services;

// 为你的 CLI 安装 hook
bool success = ConfigInstaller.InstallPlugin("my-cli");

// 检查是否已安装
bool installed = ConfigInstaller.IsPluginInstalled("my-cli");

// 卸载 hook
bool removed = ConfigInstaller.UninstallPlugin("my-cli");
```

`InstallPlugin` 方法会：
1. 读取插件定义
2. 展开路径（`~/`、环境变量）
3. 创建配置文件
4. 合并 CodeOrbit hook 条目
5. 保留现有的用户条目

### 启动 Runtime

启动 CodeOrbit Runtime，插件会自动加载：

```bash
cargo run -p codeorbit-host -- --token dev-token
```

你的 CLI 源会出现在 `/api/sources` 端点中，并可在运行时自动检测。

---

## 功能特性

### 1. 自动 CLI 检测

插件可以定义检测规则来自动识别正在运行的 CLI：

```json
{
  "detection": {
    "process_names": ["my-cli", "mycli-agent"],
    "env_var_hints": {
      "MY_CLI_HOME": "*",
      "MY_CLI_VERSION": "2.*"
    },
    "path_patterns": ["*my-cli*"],
    "priority": 150
  }
}
```

**工作原理**：
- Bridge 收集进程族谱（父进程、祖父进程等）
- Runtime 按检测规则匹配（最高优先级优先）
- 第一个匹配的插件获胜
- 无需手动 `--source` 参数

**优先级级别**：
- **1000+**：内置插件（Claude、Codex 等内置 CLI）
- **500-999**：高优先级用户插件
- **100-499**：普通优先级用户插件
- **1-99**：低优先级用户插件

### 2. Hook 安装

插件指定如何将 hook 安装到 CLI 的配置中：

```json
{
  "hook_installation": {
    "format": "flat",
    "config_path": "~/.my-cli/hooks.json",
    "events": ["PreToolUse", "PostToolUse", "SessionStart"],
    "timeout_seconds": 10
  }
}
```

**支持的格式**：
- **flat**：数组格式 `[{event, command, timeout}]`（Cursor、Trae）
- **nested**：嵌套格式 `{hooks: {Event: [{command, timeout}]}}`（Gemini、Kiro、OpenCode 等）
- **codex**：Codex 双层嵌套格式，支持 `commandWindows` 和长超时
- **claude-matcher**：支持 matcher 的 Claude 格式
- **copilot**：GitHub Copilot 的 `{version, hooks: [...]}` 格式
- **cline**：Cline 的 per-event PowerShell 脚本目录格式

### 3. 额外配置

某些 CLI 除了 hook 外还需要额外配置：

```json
{
  "hook_installation": {
    "format": "nested",
    "config_path": "~/.my-cli/hooks.json",
    "events": ["PreToolUse", "PostToolUse"],
    "timeout_seconds": 10,
    "extra_config": {
      "file": "~/.my-cli/config.toml",
      "section": "[features]",
      "key": "hooks",
      "value": "true"
    }
  }
}
```

这确保 CLI 的 hook 系统已启用（例如 Codex 需要 config.toml 中的 `hooks = true`）。

---

## 插件类型

### 内置插件

Runtime 自带的插件（位于 `bundled-plugins/`）：
- **Claude Code**：12 个事件，claude-matcher 格式
- **Codex CLI**：7 个事件，codex 格式，支持 config.toml
- **GitHub Copilot**：7 个事件，copilot 格式
- **Cline**：5 个事件，cline 格式
- **以及 Cursor、Gemini CLI、Kiro、OpenCode、Qwen Code 等更多内置 CLI**

特性：
- 首先加载
- 最高优先级（用户无法覆盖）
- 保证稳定行为

### 用户插件

用户创建的插件（位于 `%AppData%\CodeOrbit\sources\`）：
- 自定义 CLI 支持
- 优先级低于内置插件
- 可以定义任何源键（除了内置的）

---

## 使用场景

### 1. 添加新 CLI 支持

支持 Runtime 尚未内置的新 AI CLI：

```json
{
  "schema_version": "2.0",
  "source": {
    "key": "new-cli",
    "display_name": "New AI CLI",
    "icon_name": "ai",
    "permission_response_style": "claude-style"
  },
  "detection": {
    "process_names": ["new-cli"],
    "priority": 100
  },
  "hook_installation": {
    "format": "flat",
    "config_path": "~/.new-cli/hooks.json",
    "events": ["PreToolUse", "PostToolUse"],
    "timeout_seconds": 10
  }
}
```

### 2. 自定义检测优先级

为你的组织覆盖检测优先级：

```json
{
  "schema_version": "2.0",
  "source": {
    "key": "enterprise-cli",
    "display_name": "Enterprise AI",
    "icon_name": "enterprise",
    "permission_response_style": "claude-style"
  },
  "detection": {
    "process_names": ["enterprise-ai"],
    "env_var_hints": {
      "ENTERPRISE_AI_TOKEN": "*"
    },
    "priority": 800
  }
}
```

### 3. 自定义 Hook 格式的 CLI

支持具有独特 hook 配置的 CLI：

```json
{
  "schema_version": "2.0",
  "source": {
    "key": "custom-cli",
    "display_name": "Custom CLI",
    "icon_name": "custom",
    "permission_response_style": "codex"
  },
  "hook_installation": {
    "format": "nested",
    "config_path": "${CUSTOM_CLI_HOME}/hooks.json",
    "events": ["SessionStart", "SessionEnd", "PreToolUse"],
    "timeout_seconds": 15,
    "extra_config": {
      "file": "${CUSTOM_CLI_HOME}/config.yaml",
      "key": "enable_hooks",
      "value": "true"
    }
  }
}
```

---

## Schema 版本

### Schema 2.0（当前版本）

**功能**：
- ✅ 自动 CLI 检测
- ✅ Hook 安装支持
- ✅ 额外配置支持
- ✅ 路径展开
- ✅ 基于优先级的匹配

**必需字段**：
- `schema_version`: `"2.0"`
- `source`: CLI 标识

**可选字段**：
- `detection`: 检测规则
- `hook_installation`: Hook 配置

### Schema 1.0（旧版）

**功能**：
- ✅ 源定义
- ✅ 事件名称映射
- ❌ 无自动检测
- ❌ 无 hook 安装

**限制**：
- 必须手动使用 `--source` 参数
- 必须手动配置 hook
- 无自动 CLI 检测

**建议**：升级到 Schema 2.0 以获得完整功能。

---

## 高级主题

### 路径展开

路径支持多种展开格式：

```json
{
  "config_path": "~/.my-cli/hooks.json"          // 波浪号展开
  "config_path": "${MY_CLI_HOME}/hooks.json"     // 环境变量
  "config_path": "%APPDATA%\\my-cli\\hooks.json" // Windows 环境变量
}
```

### 配置合并

Hook 安装会保留现有的用户条目：

**安装前**：
```json
[
  {"event": "CustomEvent", "command": "my-script.sh"}
]
```

**安装后**：
```json
[
  {"event": "CustomEvent", "command": "my-script.sh"},
  {"event": "PreToolUse", "command": "codeorbit-bridge --source my-cli", "timeout": 10}
]
```

### 安全性

所有插件都会进行安全验证：
- 路径遍历防护
- 正则灾难性回溯检查
- 资源限制（事件、模式、超时）
- 环境变量展开安全性

---

## 文档

- **[插件 Schema 参考](plugin-schema.md)**（[English](plugin-schema.en.md)）- 完整字段参考
- **[API 参考](api-reference.md)** - REST/WebSocket API 文档
- **[集成指南](integration-guide.md)** - 展示端集成模式

---

## 示例

完整的插件示例位于：
- `bundled-plugins/claude.json` - Claude Code（claude-matcher 格式）
- `bundled-plugins/codex.json` - Codex CLI（nested 格式，带额外配置）

更多示例见[插件 Schema 参考](plugin-schema.md)。

---

**最后更新**：2026-06-16  
**当前 Schema**：2.0
