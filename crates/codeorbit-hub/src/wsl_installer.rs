//! WSL hook installation support for a Windows Runtime.

use std::path::PathBuf;
use std::process::Command;

use codeorbit_core::sources::adapter_trait::SourceAdapter;
use codeorbit_core::sources::hook_installation_utils::set_bridge_executable_path;
use codeorbit_core::sources::plugin_models::HookInstallationSpec;
use codeorbit_core::sources::{hook_strategy_factory, SourcePluginLoader};

use crate::config_installer;

pub fn list_distros() -> Result<Vec<String>, String> {
    ensure_windows_runtime()?;
    let output = Command::new("wsl.exe")
        .args(["--list", "--quiet"])
        .output()
        .map_err(|e| format!("failed to run wsl.exe: {e}"))?;
    if !output.status.success() {
        return Err(command_error("wsl.exe --list --quiet", &output.stderr));
    }
    Ok(parse_distros(&decode_wsl_output(&output.stdout)))
}

pub fn default_distro() -> Result<String, String> {
    ensure_windows_runtime()?;
    if let Ok(Some(distro)) = default_distro_from_verbose() {
        return Ok(distro);
    }
    list_distros()?
        .into_iter()
        .next()
        .ok_or_else(|| "no WSL distributions found".to_string())
}

pub fn install_plugin(source_key: &str, distro: Option<&str>) -> Result<bool, String> {
    run_strategy(source_key, distro, true, |strategy, key, spec| {
        strategy.install(key, spec)
    })
}

pub fn uninstall_plugin(source_key: &str, distro: Option<&str>) -> Result<bool, String> {
    run_strategy(source_key, distro, false, |strategy, key, spec| {
        strategy.uninstall(key, spec)
    })
}

pub fn is_plugin_installed(source_key: &str, distro: Option<&str>) -> Result<bool, String> {
    run_strategy(source_key, distro, false, |strategy, key, spec| {
        strategy.is_installed(key, spec)
    })
}

fn run_strategy(
    source_key: &str,
    distro: Option<&str>,
    require_bridge: bool,
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

fn default_distro_from_verbose() -> Result<Option<String>, String> {
    let output = Command::new("wsl.exe")
        .args(["--list", "--verbose"])
        .output()
        .map_err(|e| format!("failed to run wsl.exe: {e}"))?;
    if !output.status.success() {
        return Err(command_error("wsl.exe --list --verbose", &output.stderr));
    }
    Ok(parse_default_distro_from_verbose(&decode_wsl_output(
        &output.stdout,
    )))
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
    let detail = decode_wsl_output(stderr).trim().to_string();
    if detail.is_empty() {
        format!("{command} failed")
    } else {
        format!("{command} failed: {detail}")
    }
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

fn parse_distros(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}').to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn parse_default_distro_from_verbose(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let line = line.trim().trim_start_matches('\u{feff}').trim();
        let distro = line.strip_prefix('*')?.trim();
        distro.split_whitespace().next().map(str::to_string)
    })
}

fn decode_wsl_output(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes.iter().filter(|b| **b == 0).count() > bytes.len() / 4 {
        let mut units = Vec::with_capacity(bytes.len() / 2);
        for chunk in bytes.chunks_exact(2) {
            units.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_utf16_wsl_distro_output() {
        let raw: Vec<u8> = "Ubuntu\r\ndocker-desktop\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(
            parse_distros(&decode_wsl_output(&raw)),
            vec!["Ubuntu".to_string(), "docker-desktop".to_string()]
        );
    }

    #[test]
    fn parses_default_distro_from_verbose_output() {
        let raw = "  NAME                   STATE           VERSION\r\n* Ubuntu                 Running         2\r\n  Debian                 Stopped         2\r\n";
        assert_eq!(
            parse_default_distro_from_verbose(raw),
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
}
