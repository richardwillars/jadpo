#!/usr/bin/env python3
"""Explicitly install and byte-verify the compiler-pinned JWT test dependency.

This isolated test cache is not emitted for ordinary Jadpo applications.
Generated JWT applications use the same compiler-owned manifest and lock.
"""
import argparse
import base64
import hashlib
import hmac
import io
import json
import os
import pathlib
import stat
import subprocess
import tarfile
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
CANONICAL = ROOT / "jadpo/crates/core/src/runtime/jwt"
PIN = json.loads((CANONICAL / "dependency.json").read_text())
MAX_ARCHIVE_BYTES = 1_048_576
MAX_UNPACKED_BYTES = 2_097_152
MAX_FILES = 256


def verified_archive_files(data, integrity):
    """Read a digest-verified npm tarball without extracting attacker paths."""
    if len(data) > MAX_ARCHIVE_BYTES:
        raise ValueError("JWT package archive exceeds the compiler limit")
    actual = "sha512-" + base64.b64encode(hashlib.sha512(data).digest()).decode("ascii")
    if not hmac.compare_digest(actual, integrity):
        raise ValueError("JWT package archive integrity mismatch")
    files = {}
    unpacked = 0
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        # Iterate under count/size bounds instead of loading arbitrary metadata.
        entries = 0
        for member in archive:
            entries += 1
            if entries > MAX_FILES * 2:
                raise ValueError("JWT archive entry limit exceeded")
            parts = member.name.split("/")
            if member.name.startswith("/") or "\\" in member.name or parts[0] != "package" or any(p in ("", ".", "..") for p in parts):
                raise ValueError("Unsafe JWT archive path")
            if member.isdir():
                continue
            if not member.isfile() or len(parts) < 2:
                raise ValueError("JWT archive contains a non-file entry")
            relative = "/".join(parts[1:])
            if relative in files or len(files) >= MAX_FILES:
                raise ValueError("Duplicate or excessive JWT archive files")
            unpacked += member.size
            if member.size < 0 or unpacked > MAX_UNPACKED_BYTES:
                raise ValueError("JWT package unpacked size exceeds the compiler limit")
            stream = archive.extractfile(member)
            contents = stream.read(MAX_UNPACKED_BYTES + 1)
            if len(contents) != member.size:
                raise ValueError("JWT archive entry length mismatch")
            files[relative] = contents
    if "package.json" not in files:
        raise ValueError("JWT package metadata is missing")
    return files


def safe_file_bytes(path, limit):
    """No following symlinks, devices or pipes in a dependency inventory."""
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError("JWT dependency contains a non-regular file")
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
    with os.fdopen(descriptor, "rb") as handle:
        if not stat.S_ISREG(os.fstat(handle.fileno()).st_mode):
            raise ValueError("JWT dependency is not a regular file")
        result = handle.read(limit + 1)
    if len(result) > limit:
        raise ValueError("JWT dependency file exceeds expected size")
    return result


def verify_installed_files(package_root, expected):
    package_root = pathlib.Path(package_root)
    if package_root.is_symlink() or not package_root.is_dir():
        raise ValueError("JWT package directory is missing or a symlink")
    directories = {"."}
    for filename in expected:
        directories.update(str(parent) for parent in pathlib.PurePosixPath(filename).parents)
    found = set()
    for current, children, filenames in os.walk(package_root, followlinks=False):
        current = pathlib.Path(current)
        for child in children:
            path = current / child
            relative = path.relative_to(package_root).as_posix()
            if path.is_symlink() or relative not in directories:
                raise ValueError("Unexpected JWT dependency directory or symlink")
        for filename in filenames:
            path = current / filename
            relative = path.relative_to(package_root).as_posix()
            if relative not in expected:
                raise ValueError("Unexpected JWT dependency file")
            if safe_file_bytes(path, len(expected[relative])) != expected[relative]:
                raise ValueError("JWT installed file differs from the pinned archive")
            found.add(relative)
    if found != set(expected):
        raise ValueError("JWT dependency file inventory is incomplete")


def registry_archive(pin, cache):
    cache = pathlib.Path(cache)
    if cache.exists() or cache.is_symlink():
        return safe_file_bytes(cache, MAX_ARCHIVE_BYTES)
    if not pin["tarball"].startswith("https://registry.npmjs.org/jose/-/jose-"):
        raise ValueError("JWT package source must be the pinned official registry")
    cache.parent.mkdir(parents=True, exist_ok=True)
    # curl bounds total elapsed transfer time, including slow streaming, and
    # enforces size even when the response omits Content-Length.
    result = subprocess.run(["curl", "--fail", "--silent", "--show-error", "--proto", "=https",
        "--max-time", "30", "--max-filesize", str(MAX_ARCHIVE_BYTES), pin["tarball"]],
        check=True, capture_output=True, timeout=35)
    verified_archive_files(result.stdout, pin["integrity"])
    with tempfile.NamedTemporaryFile(dir=cache.parent, prefix="jwt-download-", delete=False) as temporary:
        temporary.write(result.stdout)
        staged = pathlib.Path(temporary.name)
    try:
        # Do not replace a concurrently created cache entry.
        os.link(staged, cache)
    except FileExistsError:
        pass
    finally:
        staged.unlink()
    return safe_file_bytes(cache, MAX_ARCHIVE_BYTES)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=pathlib.Path, default=ROOT / "build/validation/jwt-dependencies")
    args = parser.parse_args()
    root = args.directory.absolute()
    if root.is_symlink():
        raise SystemExit("Refusing a symlink as the JWT installation directory")
    root.mkdir(parents=True, exist_ok=True)
    archive = ROOT / "build/validation/jwt-dependency-evidence" / ("jose-" + PIN["version"] + ".tgz")
    expected = verified_archive_files(registry_archive(PIN, archive), PIN["integrity"])
    metadata = json.loads(expected["package.json"])
    if metadata["name"] != "jose" or metadata["version"] != PIN["version"] or any(metadata.get(field) for field in ("dependencies", "optionalDependencies", "peerDependencies")):
        raise SystemExit("Pinned JWT package metadata violates the dependency boundary")
    for name in ("package.json", "bun.lock"):
        path = root / name
        rendered = (CANONICAL / name).read_bytes()
        if path.is_symlink() or (path.exists() and safe_file_bytes(path, len(rendered)) != rendered):
            raise SystemExit(f"Refusing to overwrite a different {name} in the selected directory")
        path.write_bytes(rendered)
    modules = root / "node_modules"
    if modules.is_symlink():
        raise SystemExit("Refusing a symlink node_modules directory")
    installed = modules / "jose"
    # Frozen package managers may retain a modified existing installation.
    # Reject it before invoking Bun; never silently repair a compromised cache.
    if installed.exists() or installed.is_symlink():
        verify_installed_files(installed, expected)
    subprocess.run(["bun", "install", "--frozen-lockfile", "--ignore-scripts", "--no-progress", "--cwd", str(root)], check=True)
    verify_installed_files(installed, expected)
    packages = sorted(p.name for p in modules.iterdir() if not p.name.startswith("."))
    if packages != ["jose"]:
        raise SystemExit(f"Unexpected runtime package inventory: {packages}")
    print(f"Verified explicit JWT installation: jose@{PIN['version']}; {len(expected)} exact files; zero transitive packages")


if __name__ == "__main__":
    main()
