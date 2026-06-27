# CodeOrbit Plugin Schema Reference

This document describes the JSON schema for CodeOrbit Runtime plugins (Schema Version 2.0).

## Overview

Plugins allow you to add support for new AI CLI tools without recompiling the Runtime. Each plugin is a JSON file that defines:

1. **Source Information**: CLI identity and display properties
2. **Detection Rules** (optional): How to automatically detect this CLI
3. **Hook Installation** (optional): How to install hooks into the CLI's configuration

## Schema Version

Current version: **2.0**

```json
{
  "schema_version": "2.0"
}
```

---

## Complete Example

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

## Field Reference

### 1. `source` (required)

Defines the CLI identity and display properties.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `key` | string | ✅ | Unique identifier (lowercase, alphanumeric + hyphens) |
| `display_name` | string | ✅ | Human-readable name shown in UI |
| `icon_name` | string | ✅ | Icon identifier for display clients |
| `permission_response_style` | string | ✅ | Response format: `"claude-style"` or `"codex"` |

**Example**:

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

**Validation**:
- `key`: 1-50 chars, lowercase letters, digits, hyphens only
- `display_name`: 1-100 chars
- `icon_name`: 1-50 chars
- `permission_response_style`: must be `"claude-style"` or `"codex"`

---

### 2. `detection` (optional)

Defines rules for automatically detecting when this CLI is running.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `process_names` | string[] | ❌ | Process names to match (case-insensitive, without .exe) |
| `env_var_hints` | object | ❌ | Environment variables to check (key: glob pattern) |
| `path_patterns` | string[] | ❌ | Glob patterns to match executable paths |
| `priority` | number | ❌ | Detection priority (1-999 for plugins, 1000+ reserved) |

**Example**:

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

**How Detection Works**:

1. Bridge collects process ancestry (parent, grandparent, etc.)
2. Runtime checks detection rules in priority order (highest first)
3. First matching plugin wins
4. User can still override with `--source` parameter

**Priority Guidelines**:
- **1000+**: Reserved for bundled plugins (built-in CLIs)
- **500-999**: High priority user plugins
- **100-499**: Normal priority user plugins
- **1-99**: Low priority user plugins

**Validation**:
- `process_names`: max 50 items, each 1-100 chars
- `env_var_hints`: max 20 items, keys 1-100 chars
- `path_patterns`: max 10 items, each 1-200 chars, must be valid glob patterns
- `priority`: 1-999 for user plugins

---

### 3. `hook_installation` (optional)

Defines how to install hooks into the CLI's configuration.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `format` | string | ✅ | Hook format: `"flat"`, `"nested"`, `"codex"`, `"claude-matcher"`, `"copilot"`, or `"cline"` |
| `config_path` | string | ✅ | Path to config file or directory (`cline` uses a directory; supports `~/` and env vars) |
| `events` | string[] | ✅ | Hook events to install |
| `timeout_seconds` | number | ✅ | Default timeout for hook execution (1-86400) |
| `extra_config` | object | ❌ | Additional config file to modify (e.g., Codex config.toml) |

**Example**:

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

#### 3.1 Hook Formats

##### **Flat Format** (`"flat"`)

Used by: Cursor, Trae

Structure: Array of hook objects

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

##### **Nested Format** (`"nested"`)

Used by: Gemini, Kiro, OpenCode, and similar CLIs

Structure: Nested object with event arrays

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

##### **Codex Format** (`"codex"`)

Used by: Codex CLI

Structure: Double-nested object. Each event contains an entry with a `hooks` array.

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "codeorbit-bridge --source codex",
            "commandWindows": "C:\\path\\to\\bridge.exe --source codex",
            "timeout": 86400,
            "statusMessage": "Running hook"
          }
        ]
      }
    ]
  }
}
```

##### **Claude Matcher Format** (`"claude-matcher"`)

Used by: Claude Code

Structure: Nested with matcher support

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

##### **Copilot Format** (`"copilot"`)

Used by: GitHub Copilot

Structure: Versioned hook array.

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

##### **Cline Format** (`"cline"`)

Used by: Cline

Structure: `config_path` points to a hook script directory. One PowerShell script is written per event.

```text
%USERPROFILE%/Documents/Cline/Hooks/
├── SessionStart.ps1
├── UserPromptSubmit.ps1
├── PreToolUse.ps1
├── PostToolUse.ps1
└── Stop.ps1
```

---

#### 3.2 Supported Events

| Event | Description |
|-------|-------------|
| `SessionStart` | Session begins |
| `SessionEnd` | Session ends |
| `UserPromptSubmit` | User submits a prompt |
| `PreToolUse` | Before tool execution |
| `PostToolUse` | After successful tool execution |
| `PostToolUseFailure` | After failed tool execution |
| `PermissionRequest` | Permission required |
| `Stop` | Session stopped |
| `SubagentStart` | Subagent starts |
| `SubagentStop` | Subagent stops |
| `Notification` | Generic notification |
| `PreCompact` | Before context compaction |

**Note**: Not all CLIs support all events. Check the CLI's documentation.

---

#### 3.3 Extra Config

Some CLIs require additional configuration beyond the hooks file. Use `extra_config` to specify these.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file` | string | ✅ | Path to config file |
| `section` | string | ❌ | Section heading (e.g., `"[features]"`) |
| `key` | string | ✅ | Config key to set |
| `value` | string | ✅ | Config value to set |

**Example** (Codex config.toml):

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

This will ensure `hooks = true` exists in the `[features]` section of config.toml.

---

## Path Expansion

Paths in `config_path` and `extra_config.file` support:

- **Tilde expansion**: `~/` → User home directory
- **Environment variables**: `${HOME}`, `${MY_VAR}`, `%APPDATA%`

**Examples**:
- `~/.my-cli/hooks.json` → `C:\Users\Username\.my-cli\hooks.json` (Windows)
- `${MY_CLI_HOME}/hooks.json` → Expands `MY_CLI_HOME` env var
- `%APPDATA%\my-cli\hooks.json` → Windows AppData directory

---

## Security & Validation

All plugin fields are validated for security:

### Path Security
- ❌ Absolute path traversal: `../../../etc/passwd`
- ❌ Windows drive paths: `C:\Windows\System32`
- ✅ Tilde paths: `~/.my-cli/hooks.json`
- ✅ Relative to user home: `~/.config/my-cli/hooks.json`
- ✅ Environment variables: `${MY_CLI_HOME}/hooks.json`

### Pattern Security
- Regex patterns are checked for catastrophic backtracking
- Max nesting depth: 3
- Max repeat quantifiers: 5
- Max pattern length: 200 chars

### Resource Limits
- Process names: ≤ 50
- Environment variables: ≤ 20
- Path patterns: ≤ 10
- Events: ≤ 20
- Timeout: 1-86400 seconds
- Config path: ≤ 500 chars

---

## Plugin Location

### Bundled Plugins
Located in Runtime's `bundled-plugins/` directory:
- Loaded first
- Highest priority
- Cannot be overridden by user plugins

### User Plugins
Located in the platform-specific configuration directory:
- Windows: `%AppData%\CodeOrbit\sources\` (`C:\Users\<Username>\AppData\Roaming\CodeOrbit\sources\`)
- Linux/macOS: `$XDG_CONFIG_HOME/CodeOrbit/sources/` or `~/.config/CodeOrbit/sources/`
- Loaded after bundled plugins
- Can define custom CLIs

---

## Installation API

Use the REST API to install plugin hooks:

```bash
# Install hooks
curl -X POST http://127.0.0.1:32145/api/sources/my-cli/install \
  -H "Authorization: Bearer <token>"

# Uninstall hooks
curl -X POST http://127.0.0.1:32145/api/sources/my-cli/uninstall \
  -H "Authorization: Bearer <token>"
```

See [API Reference](api-reference.en.md) for the full REST/WebSocket contract.

---

## Complete Examples

### Example 1: Simple Flat Format

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

### Example 2: Nested Format with Extra Config

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

## Migration from Schema 1.0

Schema 1.0 plugins (without `detection` and `hook_installation`) are still supported but limited:

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

**Limitations**:
- No automatic detection (must use `--source legacy-cli`)
- No hook installation support
- Manual configuration required

**Recommendation**: Upgrade to Schema 2.0 for full features.

---

## Troubleshooting

### Plugin Not Loading

1. **Check JSON syntax**: Use `python -m json.tool plugin.json` to validate
2. **Check file location**: Must be in `%AppData%\CodeOrbit\sources\`
3. **Check validation errors**: Look for error logs in Runtime output
4. **Check source key**: Must be unique and not conflict with bundled plugins

### Detection Not Working

1. **Check process names**: Use Task Manager to verify exact process name
2. **Check priority**: Bundled plugins (1000+) override user plugins
3. **Test explicitly**: Use `--source my-cli` to bypass detection
4. **Check patterns**: Glob patterns are case-insensitive on Windows

### Hook Installation Fails

1. **Check config path**: Verify directory exists and is writable
2. **Check format**: Ensure format matches CLI's expected structure
3. **Check events**: Not all CLIs support all events
4. **Check timeout**: Some CLIs require longer timeouts (e.g., Codex: 86400)

---

## See Also

- [Plugin System Guide](source-plugins.en.md) - High-level overview and tutorials
- [Integration Guide](integration-guide.en.md) - How to integrate Runtime into your app
- [API Reference](api-reference.en.md) - Complete REST/WebSocket API documentation

---

**Last Updated**: 2026-06-16  
**Schema Version**: 2.0
