# Changelog

All notable changes to CodeOrbit (Rust) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-06-27

Initial Rust release — a full migration of the CodeOrbit backend from C# to a Rust workspace.

### Added
- Rust workspace with five crates: `codeorbit-contracts`, `codeorbit-core`,
  `codeorbit-hub`, `codeorbit-host` (binary), `codeorbit-bridge` (binary).
- Token-authenticated REST API and WebSocket realtime events (DTOs and event types
  kept compatible with the original contract for lossless base swap).
- Plugin/source system with 19 bundled CLI plugins and 6 hook-installation strategies
  (`claude-matcher`, `flat`, `nested`, `codex`, `copilot`, `cline`).
- Cross-platform IPC: Windows Named Pipe and Unix domain socket; little-endian
  length-prefixed message protocol (compatible with the original).
- Cross-platform process detection, ancestry traversal, and source resolution via `sysinfo`.
- RuntimeHost orchestration: CLI args, single-instance lock, owner-process monitor,
  graceful shutdown (Ctrl+C / SIGTERM).

### Enhancements over the original
- WebSocket `connection.established` welcome event carrying a `clientId`.
- `terminal.activate` realtime event broadcast on `activate-terminal`, decoupling the
  backend from any in-process desktop UI.
- `ClaudeMatcherStrategy` performs surgical install/uninstall that preserves the user's
  own non-CodeOrbit Claude hooks.

[0.1.0]: https://github.com/KelseySking/CodeOrbit-Rust/releases/tag/v0.1.0
