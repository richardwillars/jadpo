"""Offline contract tests for pinned package/source verification; no network/Bun."""
import base64
import hashlib
import importlib.util
import io
import json
import pathlib
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[2]


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / "tools" / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


installer = load("jwt_installer_test", "install-jwt-dependency.py")
upstream = load("jwt_upstream_test", "verify-jwt-upstream.py")
FILES = {
    "package.json": json.dumps({"name": "jose", "version": "1.0.0"}).encode(),
    "dist/webapi/index.js": b"export const authentic = true;\n",
    "LICENSE.md": b"MIT\n",
}


def archive(entries, prefix="package"):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as package:
        for name, contents in entries:
            member = tarfile.TarInfo(prefix + "/" + name)
            if isinstance(contents, tuple):
                member.type = contents[0]
                member.linkname = contents[1]
                package.addfile(member)
            else:
                member.size = len(contents)
                package.addfile(member, io.BytesIO(contents))
    return output.getvalue()


def integrity(data):
    return "sha512-" + base64.b64encode(hashlib.sha512(data).digest()).decode()


def install_files(root, files=FILES):
    root.mkdir(parents=True, exist_ok=True)
    for name, contents in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(contents)


class JwtDependencyTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="jadpo-jwt-offline-")
        self.root = pathlib.Path(self.temporary.name)
        self.package = self.root / "node_modules/jose"
        self.tarball = archive(FILES.items())
        self.expected = installer.verified_archive_files(self.tarball, integrity(self.tarball))
        install_files(self.package)

    def tearDown(self):
        self.temporary.cleanup()

    def test_baseline_exact_tarball_and_installed_inventory(self):
        self.assertEqual(self.expected, FILES)
        installer.verify_installed_files(self.package, self.expected)

    def test_registry_tarball_substitution_rejected_before_parsing(self):
        changed = archive({**FILES, "LICENSE.md": b"changed"}.items())
        with self.assertRaisesRegex(ValueError, "integrity mismatch"):
            installer.verified_archive_files(changed, integrity(self.tarball))

    def test_installed_file_substitution_rejected_despite_unchanged_name_version(self):
        (self.package / "dist/webapi/index.js").write_bytes(b"export const authentic = false;\n")
        self.assertEqual(json.loads((self.package / "package.json").read_text())["version"], "1.0.0")
        with self.assertRaises(ValueError):
            installer.verify_installed_files(self.package, self.expected)

    def test_unexpected_or_missing_installed_file_is_rejected(self):
        extra = self.package / "injected.js"
        extra.write_text("throw 'injected'")
        with self.assertRaisesRegex(ValueError, "Unexpected"):
            installer.verify_installed_files(self.package, self.expected)
        extra.unlink()
        (self.package / "LICENSE.md").unlink()
        with self.assertRaisesRegex(ValueError, "incomplete"):
            installer.verify_installed_files(self.package, self.expected)

    def test_file_and_directory_symlinks_cannot_escape_installation(self):
        outside = self.root / "outside"
        outside.write_bytes(FILES["LICENSE.md"])
        path = self.package / "LICENSE.md"
        path.unlink()
        path.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "non-regular"):
            installer.verify_installed_files(self.package, self.expected)
        path.unlink()
        path.write_bytes(FILES["LICENSE.md"])
        (self.package / "outside-directory").symlink_to(self.root, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            installer.verify_installed_files(self.package, self.expected)

    def test_package_root_symlink_is_rejected(self):
        alias = self.root / "alias"
        alias.symlink_to(self.package, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "symlink"):
            installer.verify_installed_files(alias, self.expected)

    def test_archive_path_escape_and_duplicate_members_are_rejected(self):
        for name in ["../escape", "/escape", "dist/../../escape", "dist\\escape", "./escape"]:
            data = archive(list(FILES.items()) + [(name, b"bad")])
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "Unsafe"):
                installer.verified_archive_files(data, integrity(data))
        data = archive(list(FILES.items()) + [("package.json", b"{}")])
        with self.assertRaisesRegex(ValueError, "Duplicate"):
            installer.verified_archive_files(data, integrity(data))

    def test_archive_symlinks_and_hardlinks_are_rejected(self):
        for kind in [tarfile.SYMTYPE, tarfile.LNKTYPE]:
            data = archive(list(FILES.items()) + [("link", (kind, "../../outside"))])
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, "non-file"):
                installer.verified_archive_files(data, integrity(data))

    def test_archive_resource_limits_are_enforced(self):
        with patch.object(installer, "MAX_ARCHIVE_BYTES", len(self.tarball) - 1):
            with self.assertRaisesRegex(ValueError, "compiler limit"):
                installer.verified_archive_files(self.tarball, integrity(self.tarball))
        with patch.object(installer, "MAX_UNPACKED_BYTES", 1):
            with self.assertRaisesRegex(ValueError, "unpacked size"):
                installer.verified_archive_files(self.tarball, integrity(self.tarball))

    def test_corrupted_existing_install_never_reaches_frozen_bun_install(self):
        canonical = self.root / "canonical"
        canonical.mkdir()
        for filename in ["package.json", "bun.lock"]:
            (canonical / filename).write_text("{}\n")
        cache = self.root / "build/validation/jwt-dependency-evidence/jose-1.0.0.tgz"
        cache.parent.mkdir(parents=True)
        cache.write_bytes(self.tarball)
        (self.package / "LICENSE.md").write_bytes(b"replaced")
        pin = {"version": "1.0.0", "integrity": integrity(self.tarball)}
        with patch.object(installer, "ROOT", self.root), patch.object(installer, "CANONICAL", canonical), patch.object(installer, "PIN", pin), patch.object(sys, "argv", ["installer", "--directory", str(self.root)]), patch.object(installer.subprocess, "run") as process:
            with self.assertRaises(ValueError):
                installer.main()
            process.assert_not_called()

    def test_cached_archive_symlink_is_rejected_without_network(self):
        real = self.root / "real.tgz"
        real.write_bytes(self.tarball)
        alias = self.root / "alias.tgz"
        alias.symlink_to(real)
        with patch.object(installer.subprocess, "run") as process:
            with self.assertRaisesRegex(ValueError, "non-regular"):
                installer.registry_archive({}, alias)
            process.assert_not_called()

    def test_upstream_archive_and_extracted_tree_are_both_verified(self):
        data = archive(FILES.items(), prefix="jose-1.0.0")
        digest = hashlib.sha256(data).hexdigest()
        expected = upstream.upstream_files(data, digest, "jose-1.0.0")
        installer.verify_installed_files(self.package, expected)
        with self.assertRaisesRegex(ValueError, "digest"):
            upstream.upstream_files(data + b"tampered", digest, "jose-1.0.0")
        (self.package / "dist/webapi/index.js").write_bytes(b"test('always passes')")
        with self.assertRaises(ValueError):
            installer.verify_installed_files(self.package, expected)


if __name__ == "__main__":
    unittest.main()
