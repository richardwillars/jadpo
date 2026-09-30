#!/usr/bin/env python3
"""Explicitly prepare/run upstream jose RS256/ES256 conformance evidence.

Downloads an integrity-pinned upstream archive and installs test-only QUnit in
build/validation. It never changes a generated application's dependencies.
The selected original upstream cases are not the full jose test suite.
"""
import argparse
import hashlib
import json
import pathlib
import subprocess
import tarfile
import io
import importlib.util

ROOT = pathlib.Path(__file__).resolve().parents[1]
PIN = json.loads((ROOT / "jadpo/crates/core/src/runtime/jwt/dependency.json").read_text())
BASE = ROOT / "build/validation"


# Test tooling only. These packages never enter generated JWT applications.
TEST_LOCK = {'lockfileVersion': 1,
 'workspaces': {'': {'devDependencies': {'qunit': '2.26.0'}}},
 'packages': {'commander': ['commander@7.2.0',
                            '',
                            {},
                            'sha512-QrWXB+ZQSVPmIWIhtEO9H+gwHaMGYiF5ChvoJ+K9ZGHG/sVsa6yiesAD1GC/x46sET00Xlwo1u49RVVVzvcSkw=='],
              'globalyzer': ['globalyzer@0.1.0',
                             '',
                             {},
                             'sha512-40oNTM9UfG6aBmuKxk/giHn5nQ8RVz/SS4Ir6zgzOv9/qC3kKZ9v4etGTcJbEl/NyVQH7FGU7d+X1egr57Md2Q=='],
              'globrex': ['globrex@0.1.2',
                          '',
                          {},
                          'sha512-uHJgbwAMwNFf5mLst7IWLNg14x1CkeqglJb/K3doi4dw6q2IvAAmM/Y81kevy83wP+Sst+nutFTYOGg3d1lsxg=='],
              'node-watch': ['node-watch@0.7.3',
                             '',
                             {},
                             'sha512-3l4E8uMPY1HdMMryPRUAl+oIHtXtyiTlIiESNSVSNxcPfzAFzeTbXFQkZfAwBbo0B1qMSG8nUABx+Gd+YrbKrQ=='],
              'qunit': ['qunit@2.26.0',
                        '',
                        {'dependencies': {'commander': '7.2.0',
                                          'node-watch': '0.7.3',
                                          'tiny-glob': '0.2.9'},
                         'bin': {'qunit': 'bin/qunit.js'}},
                        'sha512-KSv16YomcYmiK90qTOJl3Bm5IvTf2upqQDdBQWCvSQWe94FWobnUgKOpvpvZdG7VkDt3TJSI8k8g9+GGGEd7Fw=='],
              'tiny-glob': ['tiny-glob@0.2.9',
                            '',
                            {'dependencies': {'globalyzer': '0.1.0', 'globrex': '^0.1.2'}},
                            'sha512-g/55ssRPUjShh+xkfx9UPDXqhckHEsHr4Vd9zX55oSdGZc/MD0m3sferOkwWtp98bv+kcVfEHtRJgBVJzelrzg==']}}


def installer_module():
    spec = importlib.util.spec_from_file_location("jwt_installer", ROOT / "tools/install-jwt-dependency.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def upstream_files(data, expected_digest, prefix):
    if len(data) > 5_242_880 or hashlib.sha256(data).hexdigest() != expected_digest:
        raise ValueError("Upstream archive digest/size mismatch")
    files = {}
    unpacked = 0
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for index, member in enumerate(archive):
            if index >= 2048:
                raise ValueError("Too many upstream archive entries")
            name = member.name.rstrip("/") if member.isdir() else member.name
            parts = name.split("/")
            if name.startswith("/") or "\\" in name or parts[0] != prefix or any(part in ("", ".", "..") for part in parts):
                raise ValueError("Unsafe upstream archive path")
            if member.isdir():
                continue
            if not member.isfile() or len(parts) < 2:
                raise ValueError("Unsupported upstream archive entry")
            relative = "/".join(parts[1:])
            unpacked += member.size
            if relative in files or member.size < 0 or unpacked > 10_485_760:
                raise ValueError("Duplicate or oversized upstream archive")
            contents = archive.extractfile(member).read(10_485_761)
            if len(contents) != member.size:
                raise ValueError("Upstream archive length mismatch")
            files[relative] = contents
    if not files:
        raise ValueError("Empty upstream archive")
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepare", action="store_true", help="explicitly download pinned upstream source and install test-only QUnit")
    parser.add_argument("--broad", action="store_true", help="run all three modules; Bun 1.2.20 lacks ML-DSA and this broader run is expected to expose those failures")
    args = parser.parse_args()
    evidence = BASE / "jwt-dependency-evidence"
    upstream = BASE / "jwt-upstream"
    harness = BASE / "jwt-upstream-harness"
    for directory in (evidence, upstream, harness):
        if directory.is_symlink():
            raise SystemExit("Refusing a symlink as an upstream evidence directory")
        directory.mkdir(parents=True, exist_ok=True)
    archive = evidence / f"upstream-v{PIN['version']}.tar.gz"
    source = upstream / f"jose-{PIN['version']}"
    verifier = installer_module()
    if args.prepare and not archive.exists():
        url = f"https://codeload.github.com/panva/jose/tar.gz/refs/tags/v{PIN['version']}"
        downloaded = subprocess.run(["curl", "--fail", "--silent", "--show-error", "--proto", "=https",
            "--max-time", "30", "--max-filesize", "5242880", url], check=True, capture_output=True, timeout=35).stdout
        upstream_files(downloaded, PIN["upstream_archive_sha256"], source.name)
        archive.write_bytes(downloaded)
    if not archive.exists():
        raise SystemExit("Run explicitly with --prepare to download the pinned upstream test source")
    expected = upstream_files(verifier.safe_file_bytes(archive, 5_242_880), PIN["upstream_archive_sha256"], source.name)
    if args.prepare and not source.exists():
        source.mkdir()
        for relative, contents in expected.items():
            destination = source / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(contents)
    # Every run checks both the original archive and the exact extracted tree,
    # so a retained local modification cannot become upstream conformance proof.
    verifier.verify_installed_files(source, expected)
    manifest = {"private": True, "type": "module", "devDependencies": {"qunit": "2.26.0"}}
    for filename, value in [("package.json", manifest), ("bun.lock", TEST_LOCK)]:
        expected_manifest = (json.dumps(value, indent=2) + "\n").encode()
        destination = harness / filename
        if destination.is_symlink():
            raise SystemExit("Refusing symlinked upstream harness metadata")
        if args.prepare:
            destination.write_bytes(expected_manifest)
        elif not destination.exists() or destination.read_bytes() != expected_manifest:
            raise SystemExit("Upstream harness manifest/lock differs; use explicit --prepare")
    if args.prepare:
        subprocess.run(["bun", "install", "--frozen-lockfile", "--ignore-scripts", "--no-progress", "--cwd", str(harness)], check=True)
    if not (harness / "node_modules/qunit").exists():
        raise SystemExit("Run explicitly with --prepare to install test-only QUnit")
    # The runtime under test must also match the verified release archive.
    runtime_archive = evidence / ("jose-" + PIN["version"] + ".tgz")
    runtime_files = verifier.verified_archive_files(verifier.safe_file_bytes(runtime_archive, verifier.MAX_ARCHIVE_BYTES), PIN["integrity"])
    verifier.verify_installed_files(BASE / "jwt-dependencies/node_modules/jose", runtime_files)
    modules = ["jws", "jwk", "cookbook"]
    imports = "\n".join(f"import {module} from {json.dumps((source / 'tap' / (module + '.ts')).as_uri())};" for module in modules)
    runner = f'''import QUnit from "qunit";
import * as jose from {json.dumps((BASE / 'jwt-dependencies/node_modules/jose/dist/webapi/index.js').as_uri())};
{imports}
QUnit.reporters.tap.init(QUnit); QUnit.config.autostart=false; QUnit.config.testTimeout=10000;
{'' if args.broad else 'QUnit.config.filter="/(RS256|ES256)/";'}
for (const module of [jws,jwk,cookbook]) await module(QUnit,jose,jose);
const completion=new Promise(resolve=>QUnit.done(resolve)); QUnit.start(); const result=await completion;
console.log(JSON.stringify({{upstreamTag:{json.dumps(PIN['upstream_tag'])},scope:{json.dumps('three complete upstream modules' if args.broad else 'original tests matching compiler RS256/ES256 allowlist')},result}}));
if(result.failed || !result.total)process.exit(1);
'''
    # Keep every attempt's raw test output, including broad-runtime failures.
    runner_path = harness / "reproduce.ts"
    runner_path.write_text(runner)
    output = evidence / ("upstream-broad-reproduced.log" if args.broad else "upstream-allowlist-reproduced.log")
    with output.open("w") as log:
        result = subprocess.run(["bun", "--no-install", "--env-file=/dev/null", "run", str(runner_path)], cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    print(output.read_text())
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
