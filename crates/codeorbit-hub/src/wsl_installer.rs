//! WSL hook installation support for a Windows Runtime.

use std::path::PathBuf;
use std::process::Command;

use codeorbit_core::sources::adapter_trait::SourceAdapter;
use codeorbit_core::sources::hook_installation_utils::set_bridge_executable_path;
use codeorbit_core::sources::plugin_models::HookInstallationSpec;
use codeorbit_core::sources::{SourcePluginLoader, hook_strategy_factory};

use codeorbit_contracts::{WslDistroDto, WslDistrosDto};

use crate::config_installer;

/// List user-facing WSL distros (Docker/system distros filtered out).
pub fn list_distros() -> Result<Vec<String>, String> {
    Ok(list_distros_detailed()?.distros.into_iter().map(|d| d.name).collect())
}

/// Verbose list: name / state / version / default, plus resolved defaultDistro.
pub fn list_distros_detailed() -> Result<WslDistrosDto, String> {
    ensure_windows_runtime()?;
    let output = Command::new("wsl.exe")
        .args(["--list", "--verbose"])
        .output()
        .map_err(|e| format!("failed to run wsl.exe: {e}"))?;
    if !output.status.success() {
        return Err(command_error("wsl.exe --list --verbose", &output.stderr));
    }
    Ok(parse_distros_verbose(&decode_wsl_output(&output.stdout)))
}

pub fn default_distro() -> Result<String, String> {
    ensure_windows_runtime()?;
    let detailed = list_distros_detailed()?;
    if let Some(name) = detailed.default_distro {
        return Ok(name);
    }
    detailed
        .distros
        .into_iter()
        .next()
        .map(|d| d.name)
        .ok_or_else(|| "no usable WSL distributions found".to_string())
}

/// Resolve the distro that will be used for a WSL op (explicit or default).
pub fn resolve_distro_name(distro: Option<&str>) -> Result<String, String> {
    resolve_distro(distro)
}

pub fn install_plugin(source_key: &str, distro: Option<&str>) -> Result<bool, String> {
    run_strategy(source_key, distro, true, true, |strategy, key, spec| {
        strategy.install(key, spec)
    })
}

pub fn uninstall_plugin(source_key: &str, distro: Option<&str>) -> Result<bool, String> {
    run_strategy(source_key, distro, false, true, |strategy, key, spec| {
        strategy.uninstall(key, spec)
    })
}

pub fn is_plugin_installed(source_key: &str, distro: Option<&str>) -> Result<bool, String> {
    run_strategy(source_key, distro, false, false, |strategy, key, spec| {
        strategy.is_installed(key, spec)
    })
}

fn run_strategy(
    source_key: &str,
    distro: Option<&str>,
    require_bridge: bool,
    false_is_error: bool,
    operation: impl FnOnce(
        Box<dyn codeorbit_core::sources::strategies::HookInstallationStrategy>,
        &str,
        &HookInstallationSpec,
    ) -> bool,
) -> Result<bool, String> {
    let Some(spec) = find_hook_spec(source_key) else {
        return Err(format!("Unsupported source: {source_key}"));
    };
    ensure_windows_runtime()?;
    if require_bridge && !config_installer::runtime_bridge_exe_path().exists() {
        return Err(format!(
            "missing bridge executable: {}",
            config_installer::runtime_bridge_exe_path().display()
        ));
    }

    let distro = resolve_distro(distro)?;
    if is_non_user_distro(&distro) {
        return Err(format!(
            "distro '{distro}' is not a user Linux distribution (Docker/system distros are unsupported)"
        ));
    }
    let home = wsl_home(&distro)?;
    let home_unc = wsl_home_unc(&distro, &home)?;
    let spec = spec_for_wsl(&spec, &home_unc);
    let strategy = hook_strategy_factory::create(&spec.format)
        .ok_or_else(|| format!("Unsupported hook format: {}", spec.format))?;

    let bridge = if require_bridge {
        Some(wsl_path(
            &distro,
            &config_installer::runtime_bridge_exe_path(),
        )?)
    } else {
        None
    };

    let result = config_installer::with_hook_install_lock(|| {
        if let Some(bridge) = bridge {
            set_bridge_executable_path(bridge);
        }
        let result = operation(strategy, source_key, &spec);
        set_bridge_executable_path(config_installer::runtime_bridge_exe_path().to_string_lossy());
        result
    });

    if false_is_error && !result {
        return Err(format!(
            "hook operation failed (config: {})",
            spec.config_path
        ));
    }
    Ok(result)
}

fn ensure_windows_runtime() -> Result<(), String> {
    if cfg!(windows) {
        Ok(())
    } else {
        Err("WSL operations are only supported on Windows Runtime".to_string())
    }
}

fn resolve_distro(distro: Option<&str>) -> Result<String, String> {
    match distro.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => Ok(d.to_string()),
        None => default_distro(),
    }
}

fn find_hook_spec(source_key: &str) -> Option<HookInstallationSpec> {
    let loader = SourcePluginLoader::new();
    loader
        .load_plugins()
        .iter()
        .find(|p| p.source_key().eq_ignore_ascii_case(source_key))
        .and_then(|p| p.hook_installation_spec().cloned())
}

fn wsl_home(distro: &str) -> Result<String, String> {
    let output = Command::new("wsl.exe")
        .args(["-d", distro, "--", "sh", "-lc", "printf %s \"$HOME\""])
        .output()
        .map_err(|e| format!("failed to run wsl.exe: {e}"))?;
    if !output.status.success() {
        return Err(command_error("wsl.exe home probe", &output.stderr));
    }
    let home = decode_wsl_output(&output.stdout).trim().to_string();
    if home.is_empty() {
        Err(format!("could not read WSL home for distro {distro}"))
    } else {
        Ok(home)
    }
}

fn wsl_path(distro: &str, path: &PathBuf) -> Result<String, String> {
    // `wsl -- wslpath` treats `\` as escapes, so `C:\Users\...` becomes `C:Users...`.
    // Always pass a slash-normalized Windows path; fall back to /mnt/<drive>/... if needed.
    let win = windows_path_for_wslpath(path);
    let output = Command::new("wsl.exe")
        .args(["-d", distro, "--", "wslpath", "-a", &win])
        .output()
        .map_err(|e| format!("failed to run wsl.exe: {e}"))?;
    if output.status.success() {
        let converted = decode_wsl_output(&output.stdout).trim().to_string();
        if !converted.is_empty() {
            return Ok(converted);
        }
    }
    if let Some(fallback) = windows_to_wsl_mnt_path(path) {
        return Ok(fallback);
    }
    if !output.status.success() {
        Err(command_error("wslpath", &output.stderr))
    } else {
        Err(format!(
            "could not convert path for WSL: {}",
            path.display()
        ))
    }
}

/// Normalize a Windows path for `wslpath` (backslashes → slashes).
fn windows_path_for_wslpath(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Best-effort `C:\foo` → `/mnt/c/foo` when `wslpath` is unavailable.
fn windows_to_wsl_mnt_path(path: &std::path::Path) -> Option<String> {
    let normalized = windows_path_for_wslpath(path);
    let mut chars = normalized.chars();
    let drive = chars.next()?;
    if !drive.is_ascii_alphabetic() || chars.next() != Some(':') {
        return None;
    }
    let rest = chars.as_str().trim_start_matches('/');
    Some(format!("/mnt/{}/{rest}", drive.to_ascii_lowercase()))
}

fn command_error(command: &str, stderr: &[u8]) -> String {
    let detail = sanitize_wsl_error_detail(&decode_wsl_output(stderr));
    if detail.is_empty() {
        format!("{command} failed")
    } else {
        format!("{command} failed: {detail}")
    }
}

/// Drop noisy WSL proxy/NAT warnings; keep the last useful line(s).
fn sanitize_wsl_error_detail(raw: &str) -> String {
    let lines: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !is_wsl_noise_line(l))
        .collect();
    if lines.is_empty() {
        raw.trim().to_string()
    } else {
        lines.join(" | ")
    }
}

fn is_wsl_noise_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("localhost") && (lower.contains("proxy") || lower.contains("nat"))
        || lower.contains("wsl:") && lower.contains("localhost")
}

fn spec_for_wsl(spec: &HookInstallationSpec, home_unc: &str) -> HookInstallationSpec {
    let mut spec = spec.clone();
    spec.config_path = translate_home_path(&spec.config_path, home_unc);
    if let Some(extra) = &mut spec.extra_config {
        extra.file = translate_home_path(&extra.file, home_unc);
    }
    spec
}

fn translate_home_path(path: &str, home_unc: &str) -> String {
    if let Some(rest) = strip_home_prefix(path) {
        return join_unc(home_unc, rest);
    }
    if let Some(rest) = strip_prefix(path, "%USERPROFILE%") {
        return join_unc(home_unc, rest);
    }
    if let Some(rest) = strip_prefix(path, "%APPDATA%") {
        return join_unc(&join_unc(home_unc, ".config"), rest);
    }
    path.to_string()
}

fn strip_home_prefix(path: &str) -> Option<&str> {
    path.strip_prefix("~/")
        .or_else(|| path.strip_prefix("~\\"))
        .or_else(|| path.strip_prefix("$HOME/"))
        .or_else(|| path.strip_prefix("$HOME\\"))
}

fn strip_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    path.strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('/').or_else(|| rest.strip_prefix('\\')))
}

fn join_unc(base: &str, rest: &str) -> String {
    let rest = rest.trim_start_matches(['/', '\\']);
    if rest.is_empty() {
        return base.trim_end_matches(['/', '\\']).to_string();
    }
    format!(
        "{}\\{}",
        base.trim_end_matches(['/', '\\']),
        rest.replace('/', "\\")
    )
}

fn wsl_home_unc(distro: &str, home: &str) -> Result<String, String> {
    let home = home.trim();
    if !home.starts_with('/') {
        return Err(format!("WSL home must be absolute: {home}"));
    }
    let relative = home.trim_start_matches('/').replace('/', "\\");
    if relative.is_empty() {
        Ok(format!(r"\\wsl.localhost\{distro}"))
    } else {
        Ok(format!(r"\\wsl.localhost\{distro}\{relative}"))
    }
}

/// Docker Desktop / system distros cannot host CLI hooks.
fn is_non_user_distro(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    lower == "docker-desktop"
        || lower == "docker-desktop-data"
        || lower.starts_with("docker-desktop")
        || lower.ends_with("-data") && lower.contains("docker")
}

/// Parse `wsl --list --verbose` into user distros + default.
fn parse_distros_verbose(output: &str) -> WslDistrosDto {
    let mut distros = Vec::new();
    for line in output.lines() {
        let line = line.trim().trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.to_ascii_uppercase().starts_with("NAME") {
            continue;
        }
        let is_default = line.starts_with('*');
        let body = if is_default {
            line.trim_start_matches('*').trim()
        } else {
            line
        };
        let mut parts = body.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        if is_non_user_distro(name) {
            continue;
        }
        let state = parts.next().unwrap_or("Unknown").to_string();
        let version = parts.next().and_then(|v| v.parse::<u32>().ok());
        distros.push(WslDistroDto {
            name: name.to_string(),
            state,
            version,
            is_default,
        });
    }

    // If default was a filtered Docker distro, promote first user distro.
    let has_default = distros.iter().any(|d| d.is_default);
    if !has_default {
        if let Some(first) = distros.first_mut() {
            first.is_default = true;
        }
    }
    let default_distro = distros
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.name.clone())
        .or_else(|| distros.first().map(|d| d.name.clone()));

    WslDistrosDto {
        distros,
        default_distro,
    }
}

/// Decode WSL process output that may be UTF-16 LE, UTF-8, or mixed (common on stderr).
fn decode_wsl_output(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if is_mostly_utf16_le(bytes) {
        return utf16_le_lossy(bytes);
    }
    decode_mixed_wsl_bytes(bytes)
}

fn is_mostly_utf16_le(bytes: &[u8]) -> bool {
    if bytes.len() < 2 {
        return false;
    }
    let zeros = bytes.iter().filter(|b| **b == 0).count();
    zeros > bytes.len() / 4
}

fn utf16_le_lossy(bytes: &[u8]) -> String {
    let mut units = Vec::with_capacity(bytes.len() / 2 + 1);
    for chunk in bytes.chunks_exact(2) {
        units.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    // Odd trailing byte: keep as latin1-ish via replacement in mixed path; drop here.
    String::from_utf16_lossy(&units)
}

fn looks_like_utf16_le_at(bytes: &[u8], i: usize) -> bool {
    // Need at least 2 code units of ASCII-ish UTF-16 LE: XX 00 YY 00
    if i + 4 > bytes.len() {
        return false;
    }
    bytes[i + 1] == 0
        && bytes[i + 3] == 0
        && bytes[i].is_ascii()
        && bytes[i + 2].is_ascii()
}

fn decode_mixed_wsl_bytes(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if looks_like_utf16_le_at(bytes, i) {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() && bytes[i + 1] == 0 {
                i += 2;
            }
            // If last unit incomplete, stop before it.
            out.push_str(&utf16_le_lossy(&bytes[start..i]));
        } else {
            let start = i;
            i += 1;
            while i < bytes.len() && !looks_like_utf16_le_at(bytes, i) {
                i += 1;
            }
            out.push_str(&String::from_utf8_lossy(&bytes[start..i]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utf16_wsl_distro_output_filters_docker() {
        // quiet-style names still go through the same filter used by verbose parser.
        let raw = "  NAME  STATE VERSION\r\n  Ubuntu Running 2\r\n  docker-desktop Running 2\r\n  docker-desktop-data Stopped 2\r\n  Debian Stopped 2\r\n";
        let names: Vec<_> = parse_distros_verbose(raw)
            .distros
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert_eq!(names, vec!["Ubuntu".to_string(), "Debian".to_string()]);
        let _ = raw;
        let encoded: Vec<u8> = "Ubuntu\r\ndocker-desktop\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert!(decode_wsl_output(&encoded).contains("Ubuntu"));
    }

    #[test]
    fn parses_verbose_distros_with_state_and_default() {
        let raw = "  NAME                   STATE           VERSION\r\n* Ubuntu                 Running         2\r\n  Debian                 Stopped         2\r\ndocker-desktop         Running         2\r\n";
        let dto = parse_distros_verbose(raw);
        assert_eq!(dto.distros.len(), 2);
        assert_eq!(dto.distros[0].name, "Ubuntu");
        assert_eq!(dto.distros[0].state, "Running");
        assert_eq!(dto.distros[0].version, Some(2));
        assert!(dto.distros[0].is_default);
        assert_eq!(dto.distros[1].name, "Debian");
        assert_eq!(dto.distros[1].state, "Stopped");
        assert!(!dto.distros[1].is_default);
        assert_eq!(dto.default_distro.as_deref(), Some("Ubuntu"));
    }

    #[test]
    fn parses_default_distro_skips_docker_default() {
        let raw = "  NAME                   STATE           VERSION\r\n* docker-desktop         Running         2\r\n  Ubuntu                 Running         2\r\n";
        let dto = parse_distros_verbose(raw);
        assert_eq!(dto.default_distro.as_deref(), Some("Ubuntu"));
        assert!(dto.distros[0].is_default);
        assert_eq!(dto.distros[0].name, "Ubuntu");
    }

    #[test]
    fn parses_default_distro_from_verbose_output() {
        let raw = "  NAME                   STATE           VERSION\r\n* Ubuntu                 Running         2\r\n  Debian                 Stopped         2\r\n";
        assert_eq!(
            parse_distros_verbose(raw).default_distro,
            Some("Ubuntu".to_string())
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn wsl_operations_are_windows_only() {
        assert!(list_distros().unwrap_err().contains("Windows Runtime"));
    }

    #[test]
    fn maps_wsl_home_to_unc() {
        assert_eq!(
            wsl_home_unc("Ubuntu", "/home/amiya").unwrap(),
            r"\\wsl.localhost\Ubuntu\home\amiya"
        );
    }

    #[test]
    fn translates_home_markers_to_wsl_unc() {
        let home = r"\\wsl.localhost\Ubuntu\home\amiya";
        assert_eq!(
            translate_home_path("~/.codex/hooks.json", home),
            r"\\wsl.localhost\Ubuntu\home\amiya\.codex\hooks.json"
        );
        assert_eq!(
            translate_home_path("%APPDATA%/tool/config.json", home),
            r"\\wsl.localhost\Ubuntu\home\amiya\.config\tool\config.json"
        );
    }

    #[test]
    fn normalizes_windows_path_for_wslpath() {
        assert_eq!(
            windows_path_for_wslpath(std::path::Path::new(
                r"C:\Users\amiya\AppData\Local\Programs\codeorbit-bridge.exe"
            )),
            "C:/Users/amiya/AppData/Local/Programs/codeorbit-bridge.exe"
        );
    }

    #[test]
    fn maps_windows_path_to_mnt() {
        assert_eq!(
            windows_to_wsl_mnt_path(std::path::Path::new(
                r"C:\Users\amiya\AppData\Local\Programs\codeorbit-bridge.exe"
            ))
            .as_deref(),
            Some("/mnt/c/Users/amiya/AppData/Local/Programs/codeorbit-bridge.exe")
        );
        assert_eq!(
            windows_to_wsl_mnt_path(std::path::Path::new("/home/amiya/bin")),
            None
        );
    }

    #[test]
    fn decodes_mixed_utf16_and_utf8_stderr() {
        let mut raw = Vec::new();
        // UTF-16 LE: "wsl: warn\n"
        for u in "wsl: warn\n".encode_utf16() {
            raw.extend_from_slice(&u.to_le_bytes());
        }
        // UTF-8: real error
        raw.extend_from_slice(b"wslpath: C:Users bad\n");
        let decoded = decode_wsl_output(&raw);
        assert!(decoded.contains("wslpath: C:Users bad"), "got: {decoded}");
        assert!(decoded.contains("wsl: warn"), "got: {decoded}");
    }

    #[test]
    fn sanitizes_proxy_noise_from_errors() {
        let detail = sanitize_wsl_error_detail(
            "wsl: A localhost proxy configuration was detected but not mirrored into WSL. NAT mode does not support localhost proxy.\nwslpath: C:Users bad",
        );
        assert_eq!(detail, "wslpath: C:Users bad");
    }

    #[test]
    fn rejects_docker_distro_names() {
        assert!(is_non_user_distro("docker-desktop"));
        assert!(is_non_user_distro("docker-desktop-data"));
        assert!(!is_non_user_distro("Ubuntu"));
    }
}
