#!/usr/bin/env python3
"""Offline regression tests for `generated-package-release.py` (issue #145).

Run directly:

    python3 scripts/test-generated-package-release.py

Exercises the two highest-value classes the release-automation testing
standard names: a determinism test (same input package directory ->
byte-identical prepared output) and refusal tests (the tool refuses rather
than proceeding when a required safety input is violated -- a live publish
credential present, a secret- or local-path-shaped byte, an inexact
inventory, a prepared-tree link or special entry, or an attempted
`--publish`). It also covers tamper detection between `prepare` and `check`,
and -- only when a real `npm`/`cargo` binary is available on this machine --
the real dry-run tool invocations.
"""

import importlib.util
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parent.parent


def _load(name, relative_path):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative_path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


gpr = _load("semaprax_generated_package_release", "scripts/generated-package-release.py")


def scratch_dir():
    directory = Path(tempfile.mkdtemp(prefix="semaprax-generated-package-release-"))
    return directory


def write_packed_npm_tarball(path, payload, *, mutations=(), omissions=(), extras=()):
    """Write the exact npm-style flat tarball expected by the consumer gate."""
    contents = dict(payload)
    contents.update(dict(mutations))
    for name in omissions:
        del contents[name]
    with tarfile.open(path, mode="w:gz") as archive:
        root = tarfile.TarInfo("package/")
        root.type = tarfile.DIRTYPE
        archive.addfile(root)
        for name, data in sorted(contents.items()):
            member = tarfile.TarInfo(f"package/{name}")
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))
        for name, data in extras:
            member = tarfile.TarInfo(name)
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))


def actual_toolchain_cargo():
    """Resolve a direct Cargo only for the opt-in real-tool regression.

    The production checker intentionally refuses Rustup's cargo proxy because
    its ambient Rustup state is outside the preview's authority.  Ask Rustup
    for the selected toolchain binary in this test setup, and skip rather
    than weakening that production boundary when no direct binary is known.
    """
    configured_cargo = shutil.which("cargo")
    if not configured_cargo:
        return None
    rustup = shutil.which("rustup")
    if rustup:
        try:
            result = subprocess.run(
                [rustup, "which", "cargo"],
                capture_output=True,
                text=True,
                timeout=10,
                check=False,
            )
        except OSError:
            return None
        if result.returncode != 0:
            return None
        candidate = Path(result.stdout.strip())
    else:
        candidate = Path(configured_cargo)
    if not candidate.is_absolute() or not candidate.is_file():
        return None
    try:
        gpr._reject_rustup_proxy_cargo(candidate)
    except gpr.Rejected:
        return None
    return candidate


class NpmFixtureMixin:
    def npm_package_dir(self, root):
        package_dir = root / "npm-package"
        package_dir.mkdir()
        package_json = {
            "name": "frame-payload",
            "version": "0.1.0",
            "type": "module",
            "sideEffects": False,
            "exports": {
                ".": {"types": "./semaprax.bindings.d.ts", "import": "./semaprax.bindings.js"},
                "./app.wasm": "./app.wasm",
                "./manifest": "./semaprax.api.json",
            },
            "types": "./semaprax.bindings.d.ts",
            "files": ["app.wasm", "semaprax.js", "semaprax.bindings.js", "semaprax.bindings.d.ts", "semaprax.api.json"],
            "engines": {"node": ">=22"},
        }
        (package_dir / "package.json").write_text(json.dumps(package_json) + "\n", encoding="utf-8")
        (package_dir / "app.wasm").write_bytes(b"\0asm" + b"\x01\x02\x03")
        (package_dir / "semaprax.js").write_text("export const semaprax = {};\n", encoding="utf-8")
        (package_dir / "semaprax.bindings.js").write_text("export function call() {}\n", encoding="utf-8")
        (package_dir / "semaprax.bindings.d.ts").write_text("export declare function call(): void;\n", encoding="utf-8")
        (package_dir / "semaprax.api.json").write_text(
            json.dumps({"schema": "semaprax.public-owned-data-api.v1", "exports": []}) + "\n",
            encoding="utf-8",
        )
        return package_dir


class RustFixtureMixin:
    def rust_package_dir(self, root):
        package_dir = root / "rust-package"
        package_dir.mkdir()
        (package_dir / "Cargo.toml").write_text(
            '[package]\nname = "semaprax_generated_native_rust_owned_data_sdk"\n'
            'version = "0.1.0"\nedition = "2021"\nrust-version = "1.85"\n'
            'publish = false\nbuild = "build.rs"\n\n[lib]\npath = "lib.rs"\n\n[workspace]\n',
            encoding="utf-8",
        )
        (package_dir / "build.rs").write_text("fn main() {}\n", encoding="utf-8")
        (package_dir / "lib.rs").write_text("// generated\n", encoding="utf-8")
        (package_dir / "owned_data_ffi.rs").write_text("// ffi\n", encoding="utf-8")
        (package_dir / "libsemaprax_native_rust_owned_data_sdk.a").write_bytes(b"\x7fELFfakearchive")
        (package_dir / "descriptor.json").write_text(
            json.dumps({"schema": "semaprax.public-owned-data-api.v1", "exports": []}) + "\n",
            encoding="utf-8",
        )
        (package_dir / "semaprax.native-rust-owned-data-sdk.json").write_text(
            json.dumps({"schema": "semaprax.native-rust-owned-data-sdk.v1"}) + "\n",
            encoding="utf-8",
        )
        return package_dir


class PrepareDeterminismTests(NpmFixtureMixin, RustFixtureMixin, unittest.TestCase):
    """Determinism test: `prepare` on identical input produces identical output."""

    def test_npm_prepare_is_byte_identical_across_two_runs(self):
        root = scratch_dir()
        self.addCleanup(shutil.rmtree, root, ignore_errors=True)
        package_dir = self.npm_package_dir(root)
        first = root / "first"
        second = root / "second"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", "a" * 40, first)
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", "a" * 40, second)
        self.assert_trees_identical(first, second)

    def test_rust_prepare_is_byte_identical_across_two_runs(self):
        root = scratch_dir()
        self.addCleanup(shutil.rmtree, root, ignore_errors=True)
        package_dir = self.rust_package_dir(root)
        first = root / "first"
        second = root / "second"
        gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", "a" * 40, first)
        gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", "a" * 40, second)
        self.assert_trees_identical(first, second)

    def assert_trees_identical(self, first, second):
        first_files = sorted(str(path.relative_to(first)) for path in first.rglob("*") if path.is_file())
        second_files = sorted(str(path.relative_to(second)) for path in second.rglob("*") if path.is_file())
        self.assertEqual(first_files, second_files)
        for relative in first_files:
            self.assertEqual(
                (first / relative).read_bytes(),
                (second / relative).read_bytes(),
                f"{relative} differs between two prepare runs of the same input",
            )


class PrepareRefusalTests(NpmFixtureMixin, RustFixtureMixin, unittest.TestCase):
    """Refusal tests: `prepare` fails closed on every named hazard."""

    def setUp(self):
        self.root = scratch_dir()
        self.addCleanup(shutil.rmtree, self.root, ignore_errors=True)

    def test_refuses_with_a_live_publish_credential_present(self):
        package_dir = self.npm_package_dir(self.root)
        old = os.environ.get("NPM_TOKEN")
        os.environ["NPM_TOKEN"] = "fake-token-value"
        try:
            with self.assertRaises(gpr.Rejected):
                gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, self.root / "out")
        finally:
            if old is None:
                del os.environ["NPM_TOKEN"]
            else:
                os.environ["NPM_TOKEN"] = old

    def test_refuses_a_secret_shaped_byte_in_any_file(self):
        package_dir = self.rust_package_dir(self.root)
        (package_dir / "lib.rs").write_text(
            "// ghp_" + ("a" * 36) + "\n", encoding="utf-8"
        )
        with self.assertRaises(gpr.Rejected):
            gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, self.root / "out")

    def test_refuses_a_local_host_path_leaked_into_a_file(self):
        package_dir = self.npm_package_dir(self.root)
        (package_dir / "semaprax.js").write_text(
            f"// built from {package_dir}\n", encoding="utf-8"
        )
        with self.assertRaises(gpr.Rejected):
            gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, self.root / "out")

    def test_refuses_an_extra_unexpected_file(self):
        package_dir = self.npm_package_dir(self.root)
        (package_dir / "node_modules_cache.bin").write_bytes(b"cache")
        output = self.root / "out"
        with self.assertRaisesRegex(gpr.Rejected, r"non-admitted entry"):
            gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, output)
        self.assertFalse(output.exists(), "unadmitted names must fail before creating a preview")

    def test_refuses_too_many_admitted_rust_entries_before_creating_output(self):
        package_dir = self.rust_package_dir(self.root)
        (package_dir / "semaprax_native_rust_owned_data_sdk.lib").write_bytes(b"alternate archive")
        output = self.root / "out"
        with self.assertRaisesRegex(gpr.Rejected, r"more than 7 admitted entries"):
            gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, output)
        self.assertFalse(output.exists(), "over-count input must fail before creating a preview")

    def test_refuses_a_missing_file(self):
        package_dir = self.npm_package_dir(self.root)
        (package_dir / "app.wasm").unlink()
        with self.assertRaises(gpr.Rejected):
            gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, self.root / "out")

    def test_refuses_a_rust_manifest_without_publish_false(self):
        package_dir = self.rust_package_dir(self.root)
        text = (package_dir / "Cargo.toml").read_text(encoding="utf-8")
        (package_dir / "Cargo.toml").write_text(text.replace("publish = false\n", ""), encoding="utf-8")
        with self.assertRaises(gpr.Rejected):
            gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, self.root / "out")

    def test_refuses_a_rust_manifest_with_a_dependencies_table(self):
        package_dir = self.rust_package_dir(self.root)
        text = (package_dir / "Cargo.toml").read_text(encoding="utf-8")
        (package_dir / "Cargo.toml").write_text(text + "\n[dependencies]\n", encoding="utf-8")
        with self.assertRaises(gpr.Rejected):
            gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, self.root / "out")

    def test_refuses_an_npm_package_json_with_a_forbidden_key(self):
        package_dir = self.npm_package_dir(self.root)
        parsed = json.loads((package_dir / "package.json").read_text(encoding="utf-8"))
        parsed["dependencies"] = {"left-pad": "^1.0.0"}
        (package_dir / "package.json").write_text(json.dumps(parsed), encoding="utf-8")
        with self.assertRaises(gpr.Rejected):
            gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, self.root / "out")

    def test_refuses_a_subdirectory_in_the_package(self):
        package_dir = self.npm_package_dir(self.root)
        (package_dir / "nested").mkdir()
        with self.assertRaises(gpr.Rejected):
            gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, self.root / "out")

    def test_bounded_reader_refuses_the_max_plus_one_race_case(self):
        # A pre-read stat can become stale when an input regular file grows.
        # The actual read must still take no more than max+1 bytes and refuse.
        with self.assertRaisesRegex(gpr.Rejected, r"exceeds its admitted byte limit"):
            gpr._read_bounded(io.BytesIO(b"ab"), 1, "hostile regular file")

    def test_refuses_an_oversized_payload_file_before_creating_output(self):
        package_dir = self.npm_package_dir(self.root)
        payload = max(package_dir.iterdir(), key=lambda path: path.stat().st_size)
        original = payload.read_bytes()
        payload.write_bytes(original + b"x")
        output = self.root / "out"
        with mock.patch.object(gpr, "MAX_PACKAGE_FILE_BYTES", len(original)):
            with self.assertRaisesRegex(gpr.Rejected, r"exceeds its admitted byte limit"):
                gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, output)
        self.assertFalse(output.exists(), "oversized input must fail before creating a preview")

    def test_refuses_a_payload_total_over_its_bounded_budget_before_creating_output(self):
        package_dir = self.npm_package_dir(self.root)
        total = sum(path.stat().st_size for path in package_dir.iterdir())
        output = self.root / "out"
        with mock.patch.object(gpr, "MAX_PACKAGE_FILE_BYTES", total):
            with mock.patch.object(gpr, "MAX_PACKAGE_TOTAL_BYTES", total - 1):
                with self.assertRaisesRegex(gpr.Rejected, r"exceeds its admitted byte limit"):
                    gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, output)
        self.assertFalse(output.exists(), "over-budget input must not leave a partial preview")

    def test_refuses_an_oversized_repository_license_before_creating_output(self):
        package_dir = self.npm_package_dir(self.root)
        repository = self.root / "repository"
        repository.mkdir()
        (repository / "LICENSE").write_bytes(b"xx")
        output = self.root / "out"
        with mock.patch.object(gpr, "ROOT", repository):
            with mock.patch.object(gpr, "MAX_PREVIEW_WRAPPER_BYTES", 1):
                with self.assertRaisesRegex(gpr.Rejected, r"repository LICENSE exceeds its admitted byte limit"):
                    gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, output)
        self.assertFalse(output.exists(), "oversized wrapper input must fail before creating a preview")

    def test_refuses_a_symlinked_repository_license_before_creating_output(self):
        package_dir = self.npm_package_dir(self.root)
        repository = self.root / "repository"
        repository.mkdir()
        replacement = self.root / "replacement-license"
        replacement.write_bytes(b"license\n")
        try:
            (repository / "LICENSE").symlink_to(replacement)
        except OSError as error:
            self.skipTest(f"symlink fixtures are unavailable on this host: {error}")
        output = self.root / "out"
        with mock.patch.object(gpr, "ROOT", repository):
            with self.assertRaisesRegex(
                gpr.Rejected,
                r"repository LICENSE (?:cannot be opened without following links|is a symlink)",
            ):
                gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, output)
        self.assertFalse(output.exists(), "linked wrapper input must fail before creating a preview")


class CheckTests(NpmFixtureMixin, RustFixtureMixin, unittest.TestCase):
    def setUp(self):
        self.root = scratch_dir()
        self.addCleanup(shutil.rmtree, self.root, ignore_errors=True)

    def test_publish_flag_is_always_refused(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        with self.assertRaises(gpr.Rejected):
            gpr.check("npm", prepared, publish=True)

    def test_publish_flag_is_refused_even_with_a_credential_absent_and_clean_state(self):
        # Refusal test required by the testing standard: this tool refuses to
        # proceed toward a live publish no matter how clean the input is,
        # because it implements no live-publish path at all.
        package_dir = self.rust_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, prepared)
        with self.assertRaises(gpr.Rejected):
            gpr.check("rust", prepared, publish=True)

    def test_clean_dry_run_check_succeeds_without_any_tool(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        report = gpr.check("npm", prepared, publish=False)
        self.assertTrue(any("skipped" in line for line in report))

    def test_prepared_metadata_uses_exact_lf_bytes_recorded_in_manifest(self):
        package_dir = self.rust_package_dir(self.root)
        prepared = self.root / "prepared"
        def windows_text_write(path, text, encoding=None):
            return path.write_bytes(text.replace("\n", "\r\n").encode(encoding or "utf-8"))

        with mock.patch.object(Path, "write_text", autospec=True, side_effect=windows_text_write):
            gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, prepared)
        readme = (prepared / "README.md").read_bytes()
        manifest_bytes = (prepared / "package-preview-manifest.json").read_bytes()
        self.assertIn(b"\n", readme)
        self.assertNotIn(b"\r\n", readme)
        self.assertTrue(manifest_bytes.endswith(b"\n"))
        self.assertNotIn(b"\r\n", manifest_bytes)
        manifest = json.loads(manifest_bytes)
        recorded = next(entry for entry in manifest["files"] if entry["path"] == "README.md")
        self.assertEqual(recorded["size"], len(readme))
        self.assertEqual(recorded["sha256"], gpr.sha256_hex(readme))
        gpr.check("rust", prepared, publish=False)
        (prepared / "README.md").write_bytes(readme.replace(b"\n", b"\r\n"))
        with self.assertRaisesRegex(gpr.Rejected, "on-disk file digests disagree"):
            gpr.check("rust", prepared, publish=False)

    def test_tampered_payload_file_is_detected(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        (prepared / "payload" / "semaprax.js").write_text("export const tampered = true;\n", encoding="utf-8")
        with self.assertRaises(gpr.Rejected):
            gpr.check("npm", prepared, publish=False)

    def test_tampered_readme_is_detected(self):
        package_dir = self.rust_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, prepared)
        (prepared / "README.md").write_text("tampered\n", encoding="utf-8")
        with self.assertRaises(gpr.Rejected):
            gpr.check("rust", prepared, publish=False)

    def test_check_refuses_an_oversized_prepared_payload_before_tool_dispatch(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        payload = max((prepared / "payload").iterdir(), key=lambda path: path.stat().st_size)
        original = payload.read_bytes()
        payload.write_bytes(original + b"x")
        with mock.patch.object(gpr, "MAX_PACKAGE_FILE_BYTES", len(original)):
            with mock.patch.object(gpr, "run_closed") as run_closed:
                with self.assertRaisesRegex(gpr.Rejected, r"exceeds its admitted byte limit"):
                    gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        run_closed.assert_not_called()

    def test_check_refuses_a_nonadmitted_prepared_payload_name_before_tool_dispatch(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        (prepared / "payload" / "unadmitted-cache").write_bytes(b"x")
        with mock.patch.object(gpr, "run_closed") as run_closed:
            with self.assertRaisesRegex(gpr.Rejected, r"non-admitted entry"):
                gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        run_closed.assert_not_called()

    def test_check_refuses_an_oversized_prepared_wrapper_before_tool_dispatch(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        (prepared / "README.md").write_bytes(b"xx")
        with mock.patch.object(gpr, "MAX_PREVIEW_WRAPPER_BYTES", 1):
            with mock.patch.object(gpr, "run_closed") as run_closed:
                with self.assertRaisesRegex(gpr.Rejected, r"exceeds its admitted byte limit"):
                    gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        run_closed.assert_not_called()

    def test_check_refuses_an_oversized_prepared_manifest_before_json_parsing(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        (prepared / "package-preview-manifest.json").write_bytes(b"xx")
        with mock.patch.object(gpr, "MAX_PREVIEW_MANIFEST_BYTES", 1):
            with mock.patch.object(gpr, "run_closed") as run_closed:
                with self.assertRaisesRegex(gpr.Rejected, r"exceeds its admitted byte limit"):
                    gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        run_closed.assert_not_called()

    def test_check_refuses_a_deep_prepared_manifest_before_json_or_tool_dispatch(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        depth = gpr.MAX_PREVIEW_MANIFEST_DEPTH + 1
        (prepared / "package-preview-manifest.json").write_bytes(
            b"[" * depth + b"0" + b"]" * depth
        )
        with mock.patch.object(gpr.json, "loads") as loads:
            with mock.patch.object(gpr, "run_closed") as run_closed:
                with self.assertRaisesRegex(gpr.Rejected, r"JSON nesting depth"):
                    gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        loads.assert_not_called()
        run_closed.assert_not_called()

    def test_check_refuses_a_many_node_prepared_manifest_before_json_or_tool_dispatch(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        values = gpr.MAX_PREVIEW_MANIFEST_NODES
        (prepared / "package-preview-manifest.json").write_bytes(
            b"[" + b"0," * values + b"0]"
        )
        with mock.patch.object(gpr.json, "loads") as loads:
            with mock.patch.object(gpr, "run_closed") as run_closed:
                with self.assertRaisesRegex(gpr.Rejected, r"JSON value count"):
                    gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        loads.assert_not_called()
        run_closed.assert_not_called()

    def test_resealed_npm_prepack_payload_is_refused_before_pack_tool(self):
        """A matching attacker-written checksum is not tool-run authority."""
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        package_json_path = prepared / "payload" / "package.json"
        package_json = json.loads(package_json_path.read_text(encoding="utf-8"))
        package_json["scripts"] = {"prepack": "hostile-command"}
        package_json_path.write_text(json.dumps(package_json) + "\n", encoding="utf-8")
        manifest_path = prepared / "package-preview-manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["files"] = gpr.describe_prepared(prepared, "npm")
        manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")

        with mock.patch.object(gpr, "run_closed") as run_closed:
            with self.assertRaisesRegex(gpr.Rejected, r"package\.json must not declare 'scripts'"):
                gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        run_closed.assert_not_called()

    def test_resealed_manifest_cannot_change_closed_nonclaim_or_schema(self):
        for mutation, expected in (
            (lambda manifest: manifest.update(status="published"), r"required unpublished nonclaim"),
            (lambda manifest: manifest.update(maintainer_approval=True), r"exact preview schema fields"),
        ):
            with self.subTest(expected=expected):
                case = self.root / expected.replace(" ", "-").replace("/", "-")
                case.mkdir()
                package_dir = self.npm_package_dir(case)
                prepared = case / "prepared"
                gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
                manifest_path = prepared / "package-preview-manifest.json"
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                mutation(manifest)
                manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
                with mock.patch.object(gpr, "run_closed") as run_closed:
                    with self.assertRaisesRegex(gpr.Rejected, expected):
                        gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
                run_closed.assert_not_called()

    def test_private_tool_environment_ignores_hostile_ambient_tool_config(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        ambient = self.root / "ambient-config"
        ambient.mkdir()

        def inspect_private_environment(_command, cwd, path_dirs, extra_env, **_kwargs):
            snapshot = Path(cwd).parent
            self.assertEqual(path_dirs, ["/not-reached"])
            self.assertEqual(Path(extra_env["HOME"]).parent, snapshot)
            self.assertEqual(Path(extra_env["CARGO_HOME"]).parent, snapshot)
            self.assertEqual(Path(extra_env["npm_config_cache"]).parent, snapshot)
            self.assertEqual(Path(extra_env["npm_config_userconfig"]).parent, snapshot)
            self.assertNotIn(str(ambient), extra_env.values())
            self.assertTrue(Path(extra_env["npm_config_userconfig"]).is_file())
            return SimpleNamespace(returncode=0, stderr=b"")

        hostile = {
            "HOME": str(ambient),
            "USERPROFILE": str(ambient),
            "CARGO_HOME": str(ambient),
            "npm_config_userconfig": str(ambient / "npmrc"),
            "NPM_CONFIG_CACHE": str(ambient / "npm-cache"),
        }
        with mock.patch.dict(os.environ, hostile, clear=False):
            with mock.patch.object(gpr, "run_closed", side_effect=inspect_private_environment) as run_closed:
                report = gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        run_closed.assert_called_once()
        self.assertTrue(any("npm pack --dry-run succeeded" in line for line in report))

    def test_tarball_consumer_uses_only_the_private_packed_artifact(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        calls = []
        consumer_dependencies = None

        def closed(command, cwd, path_dirs, extra_env, **_kwargs):
            nonlocal consumer_dependencies
            command = list(command)
            cwd = Path(cwd)
            calls.append((command, cwd, list(path_dirs), dict(extra_env)))
            if command[1:3] == ["pack", "--json"]:
                write_packed_npm_tarball(
                    cwd / "frame-payload-0.1.0.tgz",
                    {name: (cwd / name).read_bytes() for name in gpr.NPM_OWNED_DATA_FILES},
                )
            elif command[1:3] == ["install", "--package-lock-only"]:
                consumer_dependencies = json.loads(
                    (cwd / "package.json").read_text(encoding="utf-8")
                )["dependencies"]
                lock = {
                    "lockfileVersion": 3,
                    "packages": {
                        "": {"dependencies": {"frame-payload": "file:../payload/frame-payload-0.1.0.tgz"}},
                        "node_modules/frame-payload": {
                            "version": "0.1.0",
                            "resolved": "file:../payload/frame-payload-0.1.0.tgz",
                        },
                    },
                }
                (cwd / "package-lock.json").write_text(json.dumps(lock), encoding="utf-8")
            elif command[1] == "ci":
                installed = cwd / "node_modules" / "frame-payload"
                installed.mkdir(parents=True)
                for name in gpr.NPM_OWNED_DATA_FILES:
                    shutil.copyfile(package_dir / name, installed / name)
            return SimpleNamespace(returncode=0, stderr=b"")

        with mock.patch.object(gpr, "run_closed", side_effect=closed):
            report = gpr.check(
                "npm",
                prepared,
                publish=False,
                npm_bin=Path("/tools/npm"),
                npm_tarball_consumer=True,
                node_bin=Path("/tools/node"),
            )

        self.assertEqual(len(calls), 4)
        self.assertEqual(calls[0][0][1:3], ["pack", "--json"])
        self.assertEqual(calls[1][0][1:3], ["install", "--package-lock-only"])
        self.assertEqual(calls[2][0][1], "ci")
        self.assertEqual(calls[3][0][0], "/tools/node")
        self.assertEqual(calls[3][0][1:3], ["--input-type=module", "--eval"])
        self.assertIn('await import("frame-payload")', calls[3][0][3])
        consumer = calls[1][1]
        self.assertEqual(calls[2][1], consumer)
        self.assertEqual(calls[3][1], consumer)
        self.assertEqual(consumer_dependencies, {"frame-payload": "file:../payload/frame-payload-0.1.0.tgz"})
        for _command, cwd, _paths, _environment in calls:
            self.assertNotIn(str(package_dir), str(cwd))
            self.assertNotIn(str(prepared), str(cwd))
        self.assertTrue(any("packed artifact offline" in line for line in report))

    def test_tarball_consumer_refuses_a_substring_only_lockfile_binding_before_install(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)

        def closed(command, cwd, *_args, **_kwargs):
            command = list(command)
            cwd = Path(cwd)
            if command[1:3] == ["pack", "--json"]:
                write_packed_npm_tarball(
                    cwd / "frame-payload-0.1.0.tgz",
                    {name: (cwd / name).read_bytes() for name in gpr.NPM_OWNED_DATA_FILES},
                )
            elif command[1:3] == ["install", "--package-lock-only"]:
                (cwd / "package-lock.json").write_text(
                    json.dumps(
                        {
                            "lockfileVersion": 3,
                            "packages": {"": {"note": "file:../payload/frame-payload-0.1.0.tgz"}},
                        }
                    ),
                    encoding="utf-8",
                )
            return SimpleNamespace(returncode=0, stderr=b"")

        with mock.patch.object(gpr, "run_closed", side_effect=closed) as run_closed:
            with self.assertRaisesRegex(gpr.Rejected, r"unexpected package inventory"):
                gpr.check(
                    "npm",
                    prepared,
                    publish=False,
                    npm_bin=Path("/tools/npm"),
                    npm_tarball_consumer=True,
                    node_bin=Path("/tools/node"),
                )
        self.assertEqual(run_closed.call_count, 2)

    def test_tarball_consumer_refuses_installed_byte_substitution_before_import(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)

        def closed(command, cwd, *_args, **_kwargs):
            command = list(command)
            cwd = Path(cwd)
            if command[1:3] == ["pack", "--json"]:
                write_packed_npm_tarball(
                    cwd / "frame-payload-0.1.0.tgz",
                    {name: (cwd / name).read_bytes() for name in gpr.NPM_OWNED_DATA_FILES},
                )
            elif command[1:3] == ["install", "--package-lock-only"]:
                route = "file:../payload/frame-payload-0.1.0.tgz"
                (cwd / "package-lock.json").write_text(
                    json.dumps(
                        {
                            "lockfileVersion": 3,
                            "packages": {
                                "": {"dependencies": {"frame-payload": route}},
                                "node_modules/frame-payload": {
                                    "version": "0.1.0",
                                    "resolved": route,
                                },
                            },
                        }
                    ),
                    encoding="utf-8",
                )
            elif command[1] == "ci":
                installed = cwd / "node_modules" / "frame-payload"
                installed.mkdir(parents=True)
                for name in gpr.NPM_OWNED_DATA_FILES:
                    shutil.copyfile(package_dir / name, installed / name)
                original = (installed / "semaprax.js").read_bytes()
                (installed / "semaprax.js").write_bytes(
                    bytes([original[0] ^ 1]) + original[1:]
                )
            return SimpleNamespace(returncode=0, stderr=b"")

        with mock.patch.object(gpr, "run_closed", side_effect=closed) as run_closed:
            with self.assertRaisesRegex(gpr.Rejected, r"bytes disagree"):
                gpr.check(
                    "npm",
                    prepared,
                    publish=False,
                    npm_bin=Path("/tools/npm"),
                    npm_tarball_consumer=True,
                    node_bin=Path("/tools/node"),
                )
        self.assertEqual(run_closed.call_count, 3)

    def test_tarball_consumer_refuses_when_pack_does_not_create_one_regular_tarball(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        with mock.patch.object(
            gpr, "run_closed", return_value=SimpleNamespace(returncode=0, stderr=b"")
        ) as run_closed:
            with self.assertRaisesRegex(gpr.Rejected, r"exactly one tarball"):
                gpr.check(
                    "npm",
                    prepared,
                    publish=False,
                    npm_bin=Path("/tools/npm"),
                    npm_tarball_consumer=True,
                    node_bin=Path("/tools/node"),
                )
        run_closed.assert_called_once()

    def test_tarball_consumer_refuses_a_packed_archive_with_unchecked_bytes_before_install(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)

        def closed(command, cwd, *_args, **_kwargs):
            if list(command)[1:3] == ["pack", "--json"]:
                cwd = Path(cwd)
                write_packed_npm_tarball(
                    cwd / "frame-payload-0.1.0.tgz",
                    {name: (cwd / name).read_bytes() for name in gpr.NPM_OWNED_DATA_FILES},
                    mutations=(
                        (
                            "semaprax.js",
                            bytes([(cwd / "semaprax.js").read_bytes()[0] ^ 1])
                            + (cwd / "semaprax.js").read_bytes()[1:],
                        ),
                    ),
                )
            return SimpleNamespace(returncode=0, stderr=b"")

        with mock.patch.object(gpr, "run_closed", side_effect=closed) as run_closed:
            with self.assertRaisesRegex(gpr.Rejected, r"does not match the checked snapshot"):
                gpr.check(
                    "npm",
                    prepared,
                    publish=False,
                    npm_bin=Path("/tools/npm"),
                    npm_tarball_consumer=True,
                    node_bin=Path("/tools/node"),
                )
        run_closed.assert_called_once()

    def test_tarball_consumer_refuses_a_packed_archive_with_an_extra_member_before_install(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)

        def closed(command, cwd, *_args, **_kwargs):
            if list(command)[1:3] == ["pack", "--json"]:
                cwd = Path(cwd)
                write_packed_npm_tarball(
                    cwd / "frame-payload-0.1.0.tgz",
                    {name: (cwd / name).read_bytes() for name in gpr.NPM_OWNED_DATA_FILES},
                    omissions=("semaprax.js",),
                    extras=(("package/unadmitted.js", b"not approved"),),
                )
            return SimpleNamespace(returncode=0, stderr=b"")

        with mock.patch.object(gpr, "run_closed", side_effect=closed) as run_closed:
            with self.assertRaisesRegex(gpr.Rejected, r"unadmitted path"):
                gpr.check(
                    "npm",
                    prepared,
                    publish=False,
                    npm_bin=Path("/tools/npm"),
                    npm_tarball_consumer=True,
                    node_bin=Path("/tools/node"),
                )
        run_closed.assert_called_once()

    def test_rustup_proxy_cargo_is_refused_before_snapshot_or_tool_run(self):
        package_dir = self.rust_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, prepared)
        with mock.patch.object(gpr.os.path, "samefile", return_value=True):
            with mock.patch.object(gpr, "_write_verified_snapshot") as snapshot:
                with mock.patch.object(gpr, "run_closed") as run_closed:
                    with self.assertRaisesRegex(gpr.Rejected, r"--cargo-bin is a Rustup proxy"):
                        gpr.check("rust", prepared, publish=False, cargo_bin=Path("/not-reached/cargo"))
        snapshot.assert_not_called()
        run_closed.assert_not_called()

    def test_run_closed_windows_uses_only_system_and_explicit_tool_paths(self):
        completed = SimpleNamespace(returncode=0, stderr=b"")
        with mock.patch.object(gpr.os, "name", "nt"):
            with mock.patch.object(gpr.os, "pathsep", ";"):
                with mock.patch.dict(
                    gpr.os.environ,
                    {
                        "SystemRoot": r"C:\Windows",
                        "ComSpec": r"C:\Windows\System32\cmd.exe",
                        "HOME": r"C:\hostile-home",
                        "CARGO_HOME": r"C:\hostile-cargo",
                        "NPM_CONFIG_USERCONFIG": r"C:\hostile-npmrc",
                    },
                    clear=True,
                ):
                    with mock.patch.object(gpr.subprocess, "run", return_value=completed) as run:
                        result = gpr.run_closed(
                            [r"C:\tools\npm.cmd", "pack", "--dry-run"],
                            cwd=r"C:\snapshot\payload",
                            path_dirs=[r"C:\tools"],
                            extra_env={"HOME": r"C:\snapshot\home"},
                        )
        self.assertIs(result, completed)
        command = run.call_args.args[0]
        environment = run.call_args.kwargs["env"]
        self.assertEqual(command[:4], [r"C:\Windows\System32\cmd.exe", "/d", "/s", "/c"])
        self.assertIn(r"C:\tools\npm.cmd", command[-1])
        self.assertTrue(environment["PATH"].startswith(r"C:\tools;"))
        self.assertEqual(environment["SystemRoot"], r"C:\Windows")
        self.assertEqual(environment["ComSpec"], r"C:\Windows\System32\cmd.exe")
        self.assertEqual(environment["HOME"], r"C:\snapshot\home")
        self.assertNotIn("CARGO_HOME", environment)
        self.assertNotIn("NPM_CONFIG_USERCONFIG", environment)

    def test_same_byte_payload_symlink_is_rejected_before_pack_tool(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        original = prepared / "payload" / "semaprax.js"
        linked_bytes = self.root / "same-semaprax.js"
        linked_bytes.write_bytes(original.read_bytes())
        original.unlink()
        try:
            original.symlink_to(linked_bytes)
        except OSError as error:
            self.skipTest(f"symlink fixtures are unavailable on this host: {error}")

        # A matching byte digest alone is not enough: never let an optional
        # pack tool resolve a filesystem link outside the checked payload.
        with mock.patch.object(gpr, "run_closed") as run_closed:
            with self.assertRaisesRegex(
                gpr.Rejected,
                r"prepared payload/semaprax\.js cannot be opened without following links",
            ):
                gpr.check(
                    "npm",
                    prepared,
                    publish=False,
                    npm_bin=Path("/not-reached/npm"),
                )
        run_closed.assert_not_called()

    def test_prepared_payload_subdirectory_is_rejected_before_pack_tool(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        (prepared / "payload" / "generated-cache").mkdir()

        with mock.patch.object(gpr, "run_closed") as run_closed:
            with self.assertRaisesRegex(gpr.Rejected, r"non-admitted entry 'generated-cache'"):
                gpr.check(
                    "npm",
                    prepared,
                    publish=False,
                    npm_bin=Path("/not-reached/npm"),
                )
        run_closed.assert_not_called()

    def test_manifest_readme_and_payload_directory_symlinks_are_refused(self):
        for name in ("package-preview-manifest.json", "README.md", "payload"):
            with self.subTest(name=name):
                case = self.root / name.replace(".", "-")
                case.mkdir()
                package_dir = self.npm_package_dir(case)
                prepared = case / "prepared"
                gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
                original = prepared / name
                replacement = case / f"replacement-{name.replace('/', '-') }"
                if name == "payload":
                    shutil.copytree(original, replacement)
                    shutil.rmtree(original)
                    try:
                        original.symlink_to(replacement, target_is_directory=True)
                    except OSError as error:
                        self.skipTest(f"symlink fixtures are unavailable on this host: {error}")
                else:
                    replacement.write_bytes(original.read_bytes())
                    original.unlink()
                    try:
                        original.symlink_to(replacement)
                    except OSError as error:
                        self.skipTest(f"symlink fixtures are unavailable on this host: {error}")
                with mock.patch.object(gpr, "run_closed") as run_closed:
                    with self.assertRaisesRegex(gpr.Rejected, r"cannot be opened without following links"):
                        gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
                run_closed.assert_not_called()

    def test_named_pipe_in_payload_is_rejected_before_pack_tool(self):
        if not hasattr(os, "mkfifo"):
            self.skipTest("named-pipe fixtures are unavailable on this host")
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        os.mkfifo(prepared / "payload" / "named-pipe")
        with mock.patch.object(gpr, "run_closed") as run_closed:
            with self.assertRaisesRegex(gpr.Rejected, r"non-admitted entry 'named-pipe'"):
                gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        run_closed.assert_not_called()

    def test_swap_after_descriptor_open_uses_verified_private_snapshot(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        original = prepared / "payload" / "semaprax.js"
        expected = original.read_bytes()
        replacement = self.root / "replacement-semaprax.js"
        replacement.write_bytes(b"export const swapped_after_open = true;\n")
        real_open = gpr.os.open
        swapped = False

        def open_then_swap(*args, **kwargs):
            nonlocal swapped
            descriptor = real_open(*args, **kwargs)
            if args[0] == "semaprax.js" and kwargs.get("dir_fd") is not None and not swapped:
                swapped = True
                os.replace(replacement, original)
            return descriptor

        def inspect_snapshot(_command, cwd, **_kwargs):
            snapshot = Path(cwd)
            self.assertNotEqual(snapshot, original.parent)
            self.assertEqual((snapshot / "semaprax.js").read_bytes(), expected)
            self.assertEqual(original.read_bytes(), b"export const swapped_after_open = true;\n")
            return SimpleNamespace(returncode=0, stderr=b"")

        with mock.patch.object(gpr.os, "open", side_effect=open_then_swap):
            with mock.patch.object(gpr, "run_closed", side_effect=inspect_snapshot) as run_closed:
                report = gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        self.assertTrue(swapped)
        run_closed.assert_called_once()
        self.assertTrue(any("npm pack --dry-run succeeded" in line for line in report))

    def test_cargo_dry_run_receives_verified_private_snapshot(self):
        package_dir = self.rust_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, prepared)
        expected = (prepared / "payload" / "Cargo.toml").read_bytes()

        def inspect_snapshot(command, cwd, path_dirs, extra_env, **_kwargs):
            snapshot = Path(cwd)
            self.assertNotEqual(snapshot, prepared / "payload")
            self.assertEqual((snapshot / "Cargo.toml").read_bytes(), expected)
            self.assertEqual(Path(command[-1]), snapshot / "Cargo.toml")
            self.assertEqual(path_dirs, ["/not-reached"])
            self.assertEqual(Path(extra_env["HOME"]).parent, snapshot.parent)
            self.assertEqual(Path(extra_env["CARGO_HOME"]).parent, snapshot.parent)
            self.assertNotIn("RUSTUP_HOME", extra_env)
            return SimpleNamespace(returncode=1, stderr=b"package cannot be published")

        with mock.patch.object(gpr, "run_closed", side_effect=inspect_snapshot) as run_closed:
            report = gpr.check("rust", prepared, publish=False, cargo_bin=Path("/not-reached/cargo"))
        run_closed.assert_called_once()
        self.assertTrue(any("cargo publish --dry-run independently refused" in line for line in report))

    def test_descriptor_read_unavailable_uses_verified_private_snapshot(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        expected = (prepared / "payload" / "semaprax.js").read_bytes()

        def inspect_snapshot(_command, cwd, **_kwargs):
            snapshot = Path(cwd)
            self.assertNotEqual(snapshot, prepared / "payload")
            self.assertEqual((snapshot / "semaprax.js").read_bytes(), expected)
            return SimpleNamespace(returncode=0, stderr=b"")

        with mock.patch.object(gpr, "_descriptor_reads_available", return_value=False):
            with mock.patch.object(gpr, "run_closed", side_effect=inspect_snapshot) as run_closed:
                report = gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        run_closed.assert_called_once()
        self.assertTrue(any("npm pack --dry-run succeeded" in line for line in report))

    def test_descriptor_read_unavailable_preserves_prepare_and_check_semantics(self):
        package_dir = self.npm_package_dir(self.root)
        first = self.root / "first"
        second = self.root / "second"
        with mock.patch.object(gpr, "_descriptor_reads_available", return_value=False):
            gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", "a" * 40, first)
            gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", "a" * 40, second)
            report = gpr.check("npm", first, publish=False)
        self.assertEqual(
            sorted((path.relative_to(first), path.read_bytes()) for path in first.rglob("*") if path.is_file()),
            sorted((path.relative_to(second), path.read_bytes()) for path in second.rglob("*") if path.is_file()),
        )
        self.assertTrue(any("npm dry-run skipped" in line for line in report))

    def test_fallback_rejects_swap_after_open_before_pack_tool(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        original = prepared / "payload" / "semaprax.js"
        replacement = self.root / "fallback-replacement-semaprax.js"
        replacement.write_bytes(b"export const swapped_after_open = true;\n")
        real_open = open
        swapped = False

        def open_then_swap(path, *args, **kwargs):
            nonlocal swapped
            source = real_open(path, *args, **kwargs)
            if Path(path) == original and not swapped:
                swapped = True
                os.replace(replacement, original)
            return source

        with mock.patch.object(gpr, "_descriptor_reads_available", return_value=False):
            with mock.patch("builtins.open", side_effect=open_then_swap):
                with mock.patch.object(gpr, "run_closed") as run_closed:
                    with self.assertRaisesRegex(gpr.Rejected, r"prepared payload/semaprax\.js changed while it was read"):
                        gpr.check("npm", prepared, publish=False, npm_bin=Path("/not-reached/npm"))
        self.assertTrue(swapped)
        run_closed.assert_not_called()

    def test_check_refuses_with_a_live_publish_credential_present(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        old = os.environ.get("CARGO_REGISTRY_TOKEN")
        os.environ["CARGO_REGISTRY_TOKEN"] = "fake-token-value"
        try:
            with self.assertRaises(gpr.Rejected):
                gpr.check("npm", prepared, publish=False)
        finally:
            if old is None:
                del os.environ["CARGO_REGISTRY_TOKEN"]
            else:
                os.environ["CARGO_REGISTRY_TOKEN"] = old

    @unittest.skipUnless(shutil.which("npm"), "npm is not installed on this machine")
    def test_real_npm_pack_dry_run_succeeds_and_writes_nothing(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        before = sorted(p.name for p in (prepared / "payload").iterdir())
        report = gpr.check("npm", prepared, publish=False, npm_bin=Path(shutil.which("npm")))
        after = sorted(p.name for p in (prepared / "payload").iterdir())
        self.assertEqual(before, after, "npm pack --dry-run must not write a tarball to disk")
        self.assertTrue(any("dry-run succeeded" in line for line in report))

    @unittest.skipUnless(
        shutil.which("npm") and shutil.which("node"),
        "npm and node are not installed on this machine",
    )
    def test_real_npm_tarball_consumer_installs_verifies_and_imports_offline(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("npm", package_dir, "frame-payload", "0.1.0", None, prepared)
        report = gpr.check(
            "npm",
            prepared,
            publish=False,
            npm_bin=Path(shutil.which("npm")),
            npm_tarball_consumer=True,
            node_bin=Path(shutil.which("node")),
        )
        self.assertTrue(any("byte-verified" in line for line in report))

    def test_real_cargo_publish_dry_run_is_refused_by_cargo_itself(self):
        cargo_bin = actual_toolchain_cargo()
        if cargo_bin is None:
            self.skipTest("actual toolchain Cargo is unavailable; unresolved Rustup proxy paths are refused")
        package_dir = self.rust_package_dir(self.root)
        prepared = self.root / "prepared"
        gpr.prepare("rust", package_dir, "frame-payload", "0.1.0", None, prepared)
        report = gpr.check("rust", prepared, publish=False, cargo_bin=cargo_bin)
        self.assertTrue(any("independently refused" in line for line in report))


class GeneratedPackageSupportDecisionTests(unittest.TestCase):
    """Keep #325's policy/evidence split aligned without rebuilding packages."""

    @classmethod
    def setUpClass(cls):
        cls.adr = (ROOT / "docs/decisions/0003-maintained-generated-package-support.md").read_text(
            encoding="utf-8"
        )
        cls.package = (ROOT / "crates/semaprax-native-rust-owned-data-package/src/lib.rs").read_text(
            encoding="utf-8"
        )

    def section(self, heading):
        marker = f"## {heading}\n"
        self.assertEqual(self.adr.count(marker), 1, f"missing/duplicate ADR section: {heading}")
        return self.adr.split(marker, 1)[1].split("\n## ", 1)[0]

    def row(self, section, field):
        rows = [line for line in self.section(section).splitlines() if line.startswith(f"| {field} |")]
        self.assertEqual(len(rows), 1, f"missing/duplicate ADR field: {field}")
        return [cell.strip() for cell in rows[0].strip("|").split("|")]

    def test_current_decision_preserves_rust_only_scope_and_narrow_closure_waiver(self):
        current = " ".join(self.section("Current maintainer decision").split())
        self.assertIn("retain the accepted Rust-only scope and the eight recorded answers", current)
        self.assertIn("npm maintenance is deferred to a separate ADR", current)
        self.assertIn("No new hosted run is required solely to close #325", current)
        self.assertIn("does not choose a support policy, turn local evidence into hosted evidence", current)
        self.assertIn("Answers 5 and 7 remain", current)
        self.assertNotIn("confirmation remains pending", current)
        self.assertEqual(
            self.row("Support matrix", "npm package")[1], "Maintenance deferred"
        )

    def test_documented_identity_preview_version_and_msrv_match_frozen_generator(self):
        decision = self.section("Decision")
        for name in (
            "PUBLIC_OWNED_DATA_PROJECT_SCHEMA",
            "NATIVE_RUST_OWNED_DATA_SDK_SCHEMA",
            "OWNED_CRATE_NAME",
            "OWNED_CRATE_VERSION",
        ):
            with self.subTest(constant=name):
                match = re.search(rf'pub const {name}: &str = "([^"\n]+)";', self.package)
                self.assertIsNotNone(match, f"missing generator constant: {name}")
                self.assertIn(f"`{match.group(1)}`", decision)
        renderer = (ROOT / "crates/semaprax-native-rust-owned-data-package/src/render.rs").read_text(
            encoding="utf-8"
        )
        self.assertIn(r'rust-version = \"1.85\"', renderer)
        self.assertIn(r'publish = false', renderer)
        self.assertEqual(
            self.row("Support matrix", "Generated crate declared MSRV")[1],
            '`rust-version = "1.85"`',
        )
        version = self.row("Support matrix", "Package version scheme")[1]
        self.assertIn("Unpublished preview: fixed `0.1.0`", version)
        self.assertIn("before publication: real, incrementing SemVer plus exact API descriptor digest", version)

    def test_maintained_target_boundary_matches_exact_five_generator_targets(self):
        triples = self.package.split("pub const fn triple(self) -> &'static str {", 1)[1]
        triples = triples.split("\n    }", 1)[0]
        generated = re.findall(r'=> "([^"\n]+)"', triples)
        documented = re.findall(
            r"`([^`]+)`", self.row("Support matrix", "Maintained target boundary (5, fixed)")[1]
        )
        self.assertEqual(len(generated), 5)
        self.assertEqual(len(documented), 5)
        self.assertEqual(set(documented), set(generated))
        self.assertNotIn("aarch64-pc-windows-msvc", documented)

    def test_retained_evidence_keeps_npm_hosted_rust_and_exact_local_msrv_distinct(self):
        evidence = " ".join(self.section("Evidence").split())
        self.assertIn("not new runs performed by the 2026-09-30 decision update", evidence)
        npm = " ".join(self.row("Evidence", "14"))
        self.assertIn("77d68e49", npm)
        self.assertIn("macOS arm64", npm)
        self.assertIn("1 passed, 0 failed", npm)
        hosted = " ".join(self.row("Evidence", "15"))
        self.assertIn("a7cad038bdf09fca02b818c8753def23a1f943db", hosted)
        self.assertIn("Hosted, prior revision, Rust route only", hosted)
        self.assertIn("Not exact 1.85.0, npm execution", hosted)
        msrv = " ".join(self.row("Evidence", "16"))
        self.assertIn("0cdd26d312fd65653d24a46098195484720c78e9", msrv)
        self.assertIn("1.85.0", msrv)
        self.assertIn("aarch64-apple-darwin", msrv)
        self.assertIn("Local, unsigned, prior revision", msrv)
        self.assertIn("**1/1**", msrv)
        self.assertIn("**3/3**", msrv)
        self.assertIn("not compiler generation or archive packaging", msrv)

    def test_related_status_documents_point_to_accepted_scope_not_publication_draft(self):
        link = "decisions/0003-maintained-generated-package-support.md#current-maintainer-decision"
        stale_status = {
            "PUBLIC-OWNED-DATA-API-V1.md": "The decision remains an [unapproved draft]",
            "GENERATED-PACKAGE-PUBLICATION-DECISION-DRAFT-V1.md":
                "Closing that genuinely compiler-built npm gap still needs",
            "COMPLETION-MATRIX.md": "Make the explicit registry/support decision",
            "ROADMAP.md": "Promotion still needs a package and browser/runtime support decision",
        }
        for name, stale_claim in stale_status.items():
            with self.subTest(document=name):
                source = (ROOT / "docs" / name).read_text(encoding="utf-8")
                self.assertIn(f"]({link})", source)
                self.assertNotIn(stale_claim, " ".join(source.split()))
        draft = (ROOT / "docs/GENERATED-PACKAGE-PUBLICATION-DECISION-DRAFT-V1.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("Status: **unapproved publication design draft**", draft)
        self.assertIn("not an approved publication decision", " ".join(draft.split()))


class CliEndToEndTests(NpmFixtureMixin, unittest.TestCase):
    def setUp(self):
        self.root = scratch_dir()
        self.addCleanup(shutil.rmtree, self.root, ignore_errors=True)

    def run_cli(self, *args):
        return subprocess.run(
            [sys.executable, str(ROOT / "scripts" / "generated-package-release.py"), *args],
            capture_output=True,
            text=True,
        )

    def test_cli_prepare_then_check_succeeds(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        result = self.run_cli(
            "prepare",
            "--kind", "npm",
            "--package-dir", str(package_dir),
            "--project-name", "frame-payload",
            "--project-version", "0.1.0",
            "--output", str(prepared),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        result = self.run_cli("check", "--kind", "npm", "--prepared-dir", str(prepared))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_cli_check_publish_flag_exits_nonzero(self):
        package_dir = self.npm_package_dir(self.root)
        prepared = self.root / "prepared"
        self.run_cli(
            "prepare",
            "--kind", "npm",
            "--package-dir", str(package_dir),
            "--project-name", "frame-payload",
            "--project-version", "0.1.0",
            "--output", str(prepared),
        )
        result = self.run_cli("check", "--kind", "npm", "--prepared-dir", str(prepared), "--publish")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("rejected", result.stderr)


if __name__ == "__main__":
    unittest.main()
