#!/usr/bin/env python3
import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tarfile
import zipfile
from datetime import datetime, timezone
from pathlib import Path


PROJECT = "CodeOrbit-Rust"
BINARIES = ("codeorbit-host", "codeorbit-bridge")
DEFAULT_PORT = 32145
CONTRACT_VERSION = "1"

TARGETS = {
    "x86_64-pc-windows-msvc": ("windows-x64", "zip"),
    "aarch64-pc-windows-msvc": ("windows-arm64", "zip"),
    "x86_64-pc-windows-gnu": ("windows-x64", "zip"),
    "aarch64-pc-windows-gnullvm": ("windows-arm64", "zip"),
    "x86_64-unknown-linux-gnu": ("linux-x64", "tar.gz"),
    "aarch64-unknown-linux-gnu": ("linux-arm64", "tar.gz"),
    "x86_64-apple-darwin": ("macos-x64", "tar.gz"),
    "aarch64-apple-darwin": ("macos-arm64", "tar.gz"),
}


def run(cmd, cwd):
    print("$ " + " ".join(cmd))
    subprocess.run(cmd, cwd=cwd, check=True)


def read_version(root):
    cargo = root / "Cargo.toml"
    try:
        import tomllib

        data = tomllib.loads(cargo.read_text(encoding="utf-8"))
        return data["workspace"]["package"]["version"]
    except ModuleNotFoundError:
        in_workspace_package = False
        for line in cargo.read_text(encoding="utf-8").splitlines():
            stripped = line.strip()
            if stripped.startswith("[") and stripped.endswith("]"):
                in_workspace_package = stripped == "[workspace.package]"
            elif in_workspace_package:
                match = re.match(r'version\s*=\s*"([^"]+)"', stripped)
                if match:
                    return match.group(1)
    except KeyError as exc:
        raise SystemExit(f"missing [workspace.package].version in Cargo.toml: {exc}")
    raise SystemExit("missing [workspace.package].version in Cargo.toml")


def host_target(root):
    result = subprocess.run(
        ["rustc", "-vV"],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    for line in result.stdout.splitlines():
        if line.startswith("host: "):
            return line.removeprefix("host: ").strip()
    raise SystemExit("could not read host target from rustc -vV")


def is_windows_target(target):
    return "windows" in target


def target_info(target):
    if target in TARGETS:
        return TARGETS[target]
    sanitized = re.sub(r"[^A-Za-z0-9._-]+", "-", target).strip("-")
    return sanitized, "zip" if is_windows_target(target) else "tar.gz"


def build_dir(root, target):
    return root / "target" / target / "release" if target else root / "target" / "release"


def build(root, target):
    cmd = ["cargo", "build", "--release", "--workspace"]
    if target:
        cmd += ["--target", target]
    run(cmd, root)


def copy_payload(root, source_dir, stage_dir, target, version):
    suffix = ".exe" if is_windows_target(target) else ""
    stage_dir.mkdir(parents=True, exist_ok=True)

    for name in BINARIES:
        source = source_dir / f"{name}{suffix}"
        if not source.exists():
            raise SystemExit(f"missing build output: {source}")
        shutil.copy2(source, stage_dir / source.name)

    bundled = root / "bundled-plugins"
    if not bundled.exists():
        raise SystemExit(f"missing bundled plugins directory: {bundled}")
    shutil.copytree(bundled, stage_dir / "bundled-plugins")

    license_file = root / "LICENSE"
    if license_file.exists():
        shutil.copy2(license_file, stage_dir / "LICENSE")

    now = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    manifest = {
        "version": version,
        "runtimeVersion": version,
        "contractVersion": CONTRACT_VERSION,
        "target": target,
        "hostExe": f"codeorbit-host{suffix}",
        "bridgeExe": f"codeorbit-bridge{suffix}",
        "defaultPort": DEFAULT_PORT,
        "defaultHost": "127.0.0.1",
        "buildDate": now,
        "build_date": now,
        "default_port": DEFAULT_PORT,
        "plugin_dirs": ["bundled-plugins"],
    }
    (stage_dir / "runtime-manifest.json").write_text(
        json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )


def make_archive(stage_dir, archive_path, archive_kind):
    if archive_path.exists():
        archive_path.unlink()
    if archive_kind == "zip":
        with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
            for path in sorted(stage_dir.rglob("*")):
                if path.is_file():
                    zf.write(path, path.relative_to(stage_dir).as_posix())
    else:
        with tarfile.open(archive_path, "w:gz") as tf:
            for path in sorted(stage_dir.rglob("*")):
                tf.add(path, arcname=path.relative_to(stage_dir).as_posix())


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def package(root, release_dir, target_arg, version, host):
    target = target_arg or host
    label, archive_kind = target_info(target)
    artifact = f"{PROJECT}-v{version}-{label}"
    stage_dir = release_dir / "staging" / artifact
    if stage_dir.exists():
        shutil.rmtree(stage_dir)

    build(root, target_arg)
    copy_payload(root, build_dir(root, target_arg), stage_dir, target, version)

    archive_name = f"{artifact}.zip" if archive_kind == "zip" else f"{artifact}.tar.gz"
    archive_path = release_dir / archive_name
    make_archive(stage_dir, archive_path, archive_kind)
    print(f"created {archive_path}")
    print(f"sha256  {sha256(archive_path)}")


def main():
    parser = argparse.ArgumentParser(description="Build and package CodeOrbit release archives.")
    parser.add_argument("--target", action="append", help="Rust target triple. Repeat to build multiple packages.")
    parser.add_argument("--clean", action="store_true", help="Remove release/ before packaging.")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    release_dir = root / "release"
    if args.clean and release_dir.exists():
        shutil.rmtree(release_dir)
    release_dir.mkdir(parents=True, exist_ok=True)

    version = read_version(root)
    host = host_target(root)
    for target in args.target or [None]:
        package(root, release_dir, target, version, host)


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as exc:
        sys.exit(exc.returncode)
