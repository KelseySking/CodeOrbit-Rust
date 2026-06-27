# CodeOrbit 插件 Schema 参考

本文档描述 CodeOrbit Runtime 插件的 JSON schema（Schema 版本 2.0）。

## 概述

插件允许你在不重新编译 Runtime 的情况下添加新的 AI CLI 工具支持。每个插件是一个 JSON 文件，定义：

1. **源信息**：CLI 标识和显示属性
2. **检测规则**（可选）：如何自动检测此 CLI
3. **Hook 安装**（可选）：如何将 hook 安装到 CLI 的配置中

## Schema 版本

当前版本：**2.0**

```json
{
  "schema_version": "2.0"
}
```

---

## 完整示例

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
    "process_names": ["my-cli", "mycli"],
    "env_var_hints": {
      "MY_CLI_HOME": "*",
      "MY_CLI_VERSION": "2.*"
    },
    "path_patterns": ["*my-cli*"],
    "priority": 150
  },
  "hook_installation": {
    "format": "flat",
    "config_path": "~/.my-cli/hooks.json",
    "events": [
      "PreToolUse",
      "PostToolUse",
      "SessionStart",
      "SessionEnd"
    ],
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

---

## 字段参考

### 1. `source`（必需）

定义 CLI 标识和显示属性。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `key` | string | ✅ | 唯一标识符（小写，字母数字+连字符） |
| `display_name` | string | ✅ | UI 中显示的人类可读名称 |
| `icon_name` | string | ✅ | 展示端使用的图标标识符 |
| `permission_response_style` | string | ✅ | 响应格式：`"claude-style"` 或 `"codex"` |

**示例**：

```json
{
  "source": {
    "key": "my-cli",
    "display_name": "My AI CLI",
    "icon_name": "terminal",
    "permission_response_style": "claude-style"
  }
}
```

**验证规则**：
- `key`: 1-50 字符，仅小写字母、数字、连字符
- `display_name`: 1-100 字符
- `icon_name`: 1-50 字符
- `permission_response_style`: 必须是 `"claude-style"` 或 `"codex"`

---

### 2. `detection`（可选）

定义自动检测此 CLI 的规则。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `process_names` | string[] | ❌ | 要匹配的进程名（不区分大小写，不含 .exe） |
| `env_var_hints` | object | ❌ | 要检查的环境变量（键：glob 模式） |
| `path_patterns` | string[] | ❌ | 匹配可执行文件路径的 glob 模式 |
| `priority` | number | ❌ | 检测优先级（插件使用 1-999，1000+ 保留） |

**示例**：

```json
{
  "detection": {
    "process_names": ["my-cli", "mycli-agent"],
    "env_var_hints": {
      "MY_CLI_HOME": "*",
      "MY_CLI_VERSION": "2.*"
    },
    "path_patterns": ["*my-cli*", "*mycli*"],
    "priority": 150
  }
}
```

**检测工作原理**：

1. Bridge 收集进程族谱（父进程、祖父进程等）
2. Runtime 按优先级顺序检查检测规则（最高优先级优先）
3. 第一个匹配的插件获胜
4. 用户仍可通过 `--source` 参数覆盖

**优先级指南**：
- **1000+**：保留给内置插件（bundled plugins）
- **500-999**：高优先级用户插件
- **100-499**：普通优先级用户插件
- **1-99**：低优先级用户插件

**验证规则**：
- `process_names`: 最多 50 项，每项 1-100 字符
- `env_var_hints`: 最多 20 项，键 1-100 字符
- `path_patterns`: 最多 10 项，每项 1-200 字符，必须是有效的 glob 模式
- `priority`: 用户插件为 1-999

---

### 3. `hook_installation`（可选）

定义如何将 hook 安装到 CLI 的配置中。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `format` | string | ✅ | Hook 格式：`"flat"`, `"nested"`, `"codex"`, `"claude-matcher"`, `"copilot"`, 或 `"cline"` |
| `config_path` | string | ✅ | 配置文件或目录路径（`cline` 使用目录；支持 `~/` 和环境变量） |
| `events` | string[] | ✅ | 要安装的 hook 事件 |
| `timeout_seconds` | number | ✅ | hook 执行的默认超时时间（1-86400） |
| `extra_config` | object | ❌ | 要修改的额外配置文件（例如 Codex 的 config.toml） |

**示例**：

```json
{
  "hook_installation": {
    "format": "flat",
    "config_path": "~/.my-cli/hooks.json",
    "events": ["PreToolUse", "PostToolUse"],
    "timeout_seconds": 10
  }
}
```

---

#### 3.1 Hook 格式

##### **Flat 格式** (`"flat"`)

使用者：Cursor, Trae

结构：Hook 对象数组

```json
[
  {
    "event": "PreToolUse",
    "command": "codeorbit-bridge --source my-cli",
    "timeout": 10
  },
  {
    "event": "PostToolUse",
    "command": "codeorbit-bridge --source my-cli",
    "timeout": 10
  }
]
```

##### **Nested 格式** (`"nested"`)

使用者：Gemini

结构：嵌套对象，包含事件数组

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "command": "codeorbit-bridge --source my-cli",
        "timeout": 10
      }
    ],
    "PostToolUse": [
      {
        "command": "codeorbit-bridge --source my-cli",
        "timeout": 10
      }
    ]
  }
}
```

##### **Codex 格式** (`"codex"`)

使用者：Codex CLI

结构：双层嵌套，每个事件包含 `{hooks: [...]}` 包裹层

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "codeorbit-bridge --source my-cli",
            "commandWindows": "C:\\path\\to\\bridge.exe --source my-cli",
            "timeout": 86400,
            "statusMessage": "Running hook"
          }
        ]
      }
    ]
  }
}
```

**注意**：
- Codex 格式需要 `type: "command"` 字段
- `commandWindows` 是 Windows 特定命令（无引号，避免 cmd.exe /C 解析问题）
- `statusMessage` 在 CLI 中显示给用户
- `PreToolUse` 和 `PermissionRequest` 事件需要长超时（86400 秒）以等待用户批准

##### **Claude Matcher 格式** (`"claude-matcher"`)

使用者：Claude Code

结构：支持 matcher 的嵌套格式

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": {"type": "all"},
        "hooks": [
          {
            "type": "command",
            "command": "codeorbit-bridge --source my-cli",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
```

##### **Copilot 格式** (`"copilot"`)

使用者：GitHub Copilot

结构：带 `version` 的 hook 对象数组

```json
{
  "version": 1,
  "hooks": [
    {
      "event": "PreToolUse",
      "command": "codeorbit-bridge --source copilot",
      "timeout": 10
    }
  ]
}
```

##### **Cline 格式** (`"cline"`)

使用者：Cline

结构：`config_path` 指向 hook 脚本目录，每个事件写入一个 PowerShell 脚本。

```text
%USERPROFILE%/Documents/Cline/Hooks/
├── SessionStart.ps1
├── UserPromptSubmit.ps1
├── PreToolUse.ps1
├── PostToolUse.ps1
└── Stop.ps1
```

---

#### 3.2 支持的事件

| 事件 | 描述 |
|------|------|
| `SessionStart` | 会话开始 |
| `SessionEnd` | 会话结束 |
| `UserPromptSubmit` | 用户提交提示词 |
| `PreToolUse` | 工具执行前 |
| `PostToolUse` | 工具成功执行后 |
| `PostToolUseFailure` | 工具执行失败后 |
| `PermissionRequest` | 需要权限 |
| `Stop` | 会话停止 |
| `SubagentStart` | 子代理启动 |
| `SubagentStop` | 子代理停止 |
| `Notification` | 通用通知 |
| `PreCompact` | 上下文压缩前 |

**注意**：并非所有 CLI 都支持所有事件。请查看 CLI 的文档。

---

#### 3.3 额外配置

一些 CLI 除了 hook 文件外还需要额外配置。使用 `extra_config` 指定这些配置。

| 字段 | 类型 | 必需 | 描述 |
|------|------|------|------|
| `file` | string | ✅ | 配置文件路径 |
| `section` | string | ❌ | 配置段标题（例如 `"[features]"`） |
| `key` | string | ✅ | 要设置的配置键 |
| `value` | string | ✅ | 要设置的配置值 |

**示例**（Codex config.toml）：

```json
{
  "extra_config": {
    "file": "~/.codex/config.toml",
    "section": "[features]",
    "key": "hooks",
    "value": "true"
  }
}
```

这将确保 config.toml 的 `[features]` 段中存在 `hooks = true`。

---

## 路径展开

`config_path` 和 `extra_config.file` 中的路径支持：

- **波浪号展开**：`~/` → 用户主目录
- **环境变量**：`${HOME}`, `${MY_VAR}`, `%APPDATA%`

**示例**：
- `~/.my-cli/hooks.json` → `C:\Users\Username\.my-cli\hooks.json`（Windows）
- `${MY_CLI_HOME}/hooks.json` → 展开 `MY_CLI_HOME` 环境变量
- `%APPDATA%\my-cli\hooks.json` → Windows AppData 目录

---

## 安全与验证

所有插件字段都会进行安全验证：

### 路径安全
- ❌ 绝对路径遍历：`../../../etc/passwd`
- ❌ Windows 驱动器路径：`C:\Windows\System32`
- ✅ 波浪号路径：`~/.my-cli/hooks.json`
- ✅ 相对于用户主目录：`~/.config/my-cli/hooks.json`
- ✅ 环境变量：`${MY_CLI_HOME}/hooks.json`

### 模式安全
- 检查正则模式是否存在灾难性回溯
- 最大嵌套深度：3
- 最大重复量词：5
- 最大模式长度：200 字符

### 资源限制
- 进程名：≤ 50 个
- 环境变量：≤ 20 个
- 路径模式：≤ 10 个
- 事件：≤ 20 个
- 超时：1-86400 秒
- 配置路径：≤ 500 字符

---

## 插件位置

### 内置插件（Bundled Plugins）
位于 Runtime 的 `bundled-plugins/` 目录：
- 首先加载
- 最高优先级
- 用户插件无法覆盖

### 用户插件（User Plugins）
位于 `%AppData%\CodeOrbit\sources\`：
- Windows: `C:\Users\<用户名>\AppData\Roaming\CodeOrbit\sources\`
- 在内置插件之后加载
- 可以定义自定义 CLI

---

## 安装 API

使用 REST API 安装插件 hook：

```bash
# 安装 hook
curl -X POST http://127.0.0.1:32145/api/sources/my-cli/install \
  -H "Authorization: Bearer <token>"

# 卸载 hook
curl -X POST http://127.0.0.1:32145/api/sources/my-cli/uninstall \
  -H "Authorization: Bearer <token>"
```

完整 REST/WebSocket 接口见 [API 参考](api-reference.md)。

---

## 完整示例

### 示例 1：简单的 Flat 格式

```json
{
  "schema_version": "2.0",
  "source": {
    "key": "simple-cli",
    "display_name": "Simple CLI",
    "icon_name": "terminal",
    "permission_response_style": "claude-style"
  },
  "detection": {
    "process_names": ["simple-cli"],
    "priority": 100
  },
  "hook_installation": {
    "format": "flat",
    "config_path": "~/.simple-cli/hooks.json",
    "events": ["PreToolUse", "PostToolUse"],
    "timeout_seconds": 5
  }
}
```

### 示例 2：带额外配置的 Nested 格式

```json
{
  "schema_version": "2.0",
  "source": {
    "key": "advanced-cli",
    "display_name": "Advanced CLI",
    "icon_name": "code",
    "permission_response_style": "codex"
  },
  "detection": {
    "process_names": ["advanced-cli", "adv-cli"],
    "env_var_hints": {
      "ADV_CLI_HOME": "*"
    },
    "priority": 200
  },
  "hook_installation": {
    "format": "nested",
    "config_path": "~/.advanced-cli/hooks.json",
    "events": [
      "SessionStart",
      "SessionEnd",
      "PreToolUse",
      "PostToolUse",
      "PermissionRequest"
    ],
    "timeout_seconds": 10,
    "extra_config": {
      "file": "~/.advanced-cli/config.toml",
      "section": "[features]",
      "key": "hooks_enabled",
      "value": "true"
    }
  }
}
```

---

## 从 Schema 1.0 迁移

Schema 1.0 插件（没有 `detection` 和 `hook_installation`）仍然支持，但功能有限：

```json
{
  "schema_version": "1.0",
  "source": {
    "key": "legacy-cli",
    "display_name": "Legacy CLI",
    "icon_name": "terminal",
    "permission_response_style": "claude-style"
  }
}
```

**限制**：
- 无自动检测（必须使用 `--source legacy-cli`）
- 无 hook 安装支持
- 需要手动配置

**建议**：升级到 Schema 2.0 以获得完整功能。

---

## 故障排除

### 插件未加载

1. **检查 JSON 语法**：使用 `python -m json.tool plugin.json` 验证
2. **检查文件位置**：必须在 `%AppData%\CodeOrbit\sources\`
3. **检查验证错误**：查看 Runtime 输出中的错误日志
4. **检查源键**：必须唯一且不与内置插件冲突

### 检测不工作

1. **检查进程名**：使用任务管理器验证确切的进程名
2. **检查优先级**：内置插件（1000+）会覆盖用户插件
3. **显式测试**：使用 `--source my-cli` 绕过检测
4. **检查模式**：Glob 模式在 Windows 上不区分大小写

### Hook 安装失败

1. **检查配置路径**：验证目录存在且可写
2. **检查格式**：确保格式与 CLI 期望的结构匹配
3. **检查事件**：并非所有 CLI 都支持所有事件
4. **检查超时**：某些 CLI 需要更长的超时时间（例如 Codex: 86400）

---

## 另请参阅

- [插件系统指南](source-plugins.md) - 高级概述和教程
- [集成指南](integration-guide.md) - 如何将 Runtime 集成到你的应用
- [API 参考](api-reference.md) - 完整的 REST/WebSocket API 文档

---

**最后更新**：2026-06-16  
**Schema 版本**：2.0
