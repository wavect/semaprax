"""Structural fixtures only; synthetic ELF bytes never prove runtime execution."""
import argparse
import importlib.util
import io
import json
from pathlib import Path
import struct
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("packager", Path(__file__).parents[1] / "package-reference-service.py")
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)


def elf(segment=1, machine=62):
    data = bytearray(128)
    data[:7] = b"\x7fELF\x02\x01\x01"
    struct.pack_into("<HHI", data, 16, 2, machine, 1)
    struct.pack_into("<Q", data, 32, 64)
    struct.pack_into("<HHH", data, 52, 64, 56, 1)
    struct.pack_into("<IIQQQQQQ", data, 64, segment, 5, 0, 0, 0, 128, 128, 4096)
    return bytes(data)


class PackageTests(unittest.TestCase):
    def test_dynamic_truncated_and_wrong_platform_refuse(self):
        for data in (elf(2), elf(3), elf()[:100], elf(machine=3), b"MZ" * 64):
            with self.subTest(data=data[:20]), self.assertRaises(ValueError):
                p.linux_static_elf(data)
        with patch.object(p.platform, "system", return_value="Darwin"), self.assertRaises(ValueError):
            p.admit_executable(elf(), "development")

    def test_deterministic_layout_content_addresses_modes_and_no_secrets(self):
        files = {"bin/semaprax-reference-service": (elf(), 0o755),
                 "service/semaprax.toml": (b"fixture\n", 0o644)}
        with tempfile.TemporaryDirectory() as directory:
            roots = [Path(directory) / name for name in ("a", "b")]
            for root in roots:
                p.write_oci(root, files, "amd64")
            def inventory(root):
                return {str(path.relative_to(root)): path.read_bytes() for path in root.rglob("*") if path.is_file()}
            self.assertEqual(inventory(roots[0]), inventory(roots[1]))
            def read_blob(descriptor):
                data = (roots[0] / "blobs/sha256" / descriptor["digest"][7:]).read_bytes()
                self.assertEqual(p.digest(data), descriptor["digest"])
                self.assertEqual(len(data), descriptor["size"])
                return data
            index = json.loads((roots[0] / "index.json").read_bytes())
            manifest = json.loads(read_blob(index["manifests"][0]))
            config = json.loads(read_blob(manifest["config"]))
            self.assertEqual(config["rootfs"]["diff_ids"], [manifest["layers"][0]["digest"]])
            self.assertEqual(config["config"]["Entrypoint"], [
                "/bin/semaprax-reference-service", "serve",
                "--project", "/service", "--config", "/service/service.config.json",
                "--state-dir", "/state", "--outbound-dir", "/outbound",
                "--secrets-dir", "/secrets", "--bundle-dir", "/bundle", "--port", "8080",
            ])
            with tarfile.open(fileobj=io.BytesIO(read_blob(manifest["layers"][0]))) as archive:
                self.assertEqual({entry.name for entry in archive if entry.isfile()}, set(files))
                self.assertEqual(archive.getmember("bin/semaprax-reference-service").mode, 0o755)
                self.assertEqual(archive.extractfile("bin/semaprax-reference-service").read(), elf())

    def test_checker_failure_and_digest_mismatch_publish_nothing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in p.PROJECT_FILES:
                path = root / name
                path.parent.mkdir(exist_ok=True)
                path.write_bytes(b"not a checked project")
            executable = root / "host"
            executable.write_bytes(elf())
            config = root / "config"
            config.write_bytes(b"{}\n")
            args = argparse.Namespace(project=root, executable=executable, checker=executable,
                executable_sha256=p.digest(elf()), config=config, output=root / "out", format="oci")
            with patch.object(p.subprocess, "run", side_effect=subprocess.CalledProcessError(2, "checker")) as checker:
                with self.assertRaises(subprocess.CalledProcessError):
                    p.package(args)
                self.assertEqual(checker.call_args.args[0][1], "check-package")
            self.assertFalse(args.output.exists())
            args.executable_sha256 = "sha256:" + "0" * 64
            with patch.object(p.subprocess, "run") as checker, self.assertRaises(ValueError):
                p.package(args)
            checker.assert_not_called()
            self.assertFalse(args.output.exists())
            args.executable_sha256 = p.digest(elf())
            (root / "never-copy.secret").write_bytes(b"excluded secret fixture")
            def checked_copy(command, **kwargs):
                copied = Path(command[3])
                self.assertEqual((copied / "semaprax.toml").read_bytes(), b"not a checked project")
                self.assertFalse((copied / "never-copy.secret").exists())
            # Mocking the checker tests orchestration only; the real checker
            # accepts/refuses projects in the Rust process fixture.
            with patch.object(p.subprocess, "run", side_effect=checked_copy):
                p.package(args)
                with self.assertRaises(FileExistsError):
                    p.package(args)
            receipt = json.loads((args.output / "service-package.json").read_bytes())
            self.assertEqual(len(receipt["files"]), 6)
            self.assertIn("runtime_execution_not_performed", receipt["nonclaims"])


if __name__ == "__main__":
    unittest.main()
