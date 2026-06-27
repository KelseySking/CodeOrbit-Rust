# CLI Source Plugin System

CodeOrbit Runtime supports extending CLI sources through JSON plugin files without recompilation. The plugin system enables automatic CLI detection and hook installation.

## Overview

The plugin system provides three main capabilities:

1. **Source Definition**: Define CLI identity and display properties
2. **Automatic Detection**: Automatically detect which CLI is running based on process names, environment variables, and paths
3. **Hook Installation**: Automatically install hooks into the CLI's configuration files

## Quick Start

### Basic Plugin (Schema 2.0)

Create a JSON file (e.g., `my-cli.json`) in `%AppData%\CodeOrbit\sources\`:

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

### Install Hooks

Use `ConfigInstaller` to install hooks automatically:

```csharp
using codeorbit-core.Services;

// Install hooks for your CLI
bool success = ConfigInstaller.InstallPlugin("my-cli");

// Check if already installed
bool installed = ConfigInstaller.IsPluginInstalled("my-cli");

// Uninstall hooks
bool removed = ConfigInstaller.UninstallPlugin("my-cli");
```

The `InstallPlugin` method will:
1. Read the plugin definition
2. Expand paths (`~/`, environment variables)
3. Create the configuration file
4. Merge CodeOrbit hook entries
5. Preserve existing user entries

### Start Runtime

Start CodeOrbit Runtime, and plugins will load automatically:

```bash
cargo run -p codeorbit-host -- --token dev-token
```

Your CLI source will appear in the `/api/sources` endpoint and can be automatically detected when running.

---

## Features

### 1. Automatic CLI Detection

Plugins can define detection rules to automatically identify which CLI is running:

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

**How it works**:
- Bridge collects process ancestry (parent, grandparent, etc.)
- Runtime matches against detection rules (highest priority first)
- First matching plugin wins
- No need for manual `--source` parameter

**Priority levels**:
- **1000+**: Bundled plugins (built-in CLIs like Claude, Codex)
- **500-999**: High priority user plugins
- **100-499**: Normal priority user plugins
- **1-99**: Low priority user plugins

### 2. Hook Installation

Plugins specify how to install hooks into the CLI's configuration:

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

**Supported formats**:
- **flat**: Array format `[{event, command, timeout}]` (Cursor, Trae)
- **nested**: Nested format `{hooks: {Event: [{command, timeout}]}}` (Gemini, Kiro, OpenCode, etc.)
- **codex**: Codex double-nested format with `commandWindows` and long timeouts
- **claude-matcher**: Claude format with matcher support
- **copilot**: GitHub Copilot `{version, hooks: [...]}` format
- **cline**: Cline per-event PowerShell script directory format

### 3. Extra Configuration

Some CLIs require additional configuration beyond hooks:

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

This ensures the CLI's hook system is enabled (e.g., Codex requires `hooks = true` in config.toml).

---

## Plugin Types

### Bundled Plugins

Shipped with Runtime in `bundled-plugins/`:
- **Claude Code**: 12 events, claude-matcher format
- **Codex CLI**: 7 events, codex format with config.toml
- **GitHub Copilot**: 7 events, copilot format
- **Cline**: 5 events, cline format
- **Plus Cursor, Gemini CLI, Kiro, OpenCode, Qwen Code, and more bundled CLIs**

Properties:
- Loaded first
- Highest priority (cannot be overridden)
- Guaranteed stable behavior

### User Plugins

Created by users in `%AppData%\CodeOrbit\sources\`:
- Custom CLI support
- Lower priority than bundled plugins
- Can define any source key (except built-in ones)

---

## Use Cases

### 1. Add Support for New CLI

Support a new AI CLI that Runtime doesn't know about:

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

### 2. Customize Detection Priority

Override detection priority for your organization:

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

### 3. CLI with Custom Hook Format

Support CLI with unique hook configuration:

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

## Schema Versions

### Schema 2.0 (Current)

**Features**:
- ✅ Automatic CLI detection
- ✅ Hook installation support
- ✅ Extra config support
- ✅ Path expansion
- ✅ Priority-based matching

**Required fields**:
- `schema_version`: `"2.0"`
- `source`: CLI identity

**Optional fields**:
- `detection`: Detection rules
- `hook_installation`: Hook configuration

### Schema 1.0 (Legacy)

**Features**:
- ✅ Source definition
- ✅ Event name mapping
- ❌ No automatic detection
- ❌ No hook installation

**Limitations**:
- Must use `--source` parameter manually
- Must configure hooks manually
- No automatic CLI detection

**Recommendation**: Upgrade to Schema 2.0 for full functionality.

---

## Advanced Topics

### Path Expansion

Paths support multiple expansion formats:

```json
{
  "config_path": "~/.my-cli/hooks.json"          // Tilde expansion
  "config_path": "${MY_CLI_HOME}/hooks.json"     // Environment variable
  "config_path": "%APPDATA%\\my-cli\\hooks.json" // Windows env var
}
```

### Config Merging

Hook installation preserves existing user entries:

**Before installation**:
```json
[
  {"event": "CustomEvent", "command": "my-script.sh"}
]
```

**After installation**:
```json
[
  {"event": "CustomEvent", "command": "my-script.sh"},
  {"event": "PreToolUse", "command": "codeorbit-bridge --source my-cli", "timeout": 10}
]
```

### Security

All plugins are validated for security:
- Path traversal prevention
- Regex catastrophic backtracking checks
- Resource limits (events, patterns, timeouts)
- Environment variable expansion safety

---

## Documentation

- **[Plugin Schema Reference](plugin-schema.en.md)** ([中文](plugin-schema.md)) - Complete field reference
- **[API Reference](api-reference.en.md)** - REST/WebSocket API documentation
- **[Integration Guide](integration-guide.en.md)** - Integration patterns for display clients

---

## Examples

Complete plugin examples are available in:
- `bundled-plugins/claude.json` - Claude Code (claude-matcher format)
- `bundled-plugins/codex.json` - Codex CLI (nested format with extra config)

See [Plugin Schema Reference](plugin-schema.en.md) for more examples.

---

**Last Updated**: 2026-06-16  
**Current Schema**: 2.0
