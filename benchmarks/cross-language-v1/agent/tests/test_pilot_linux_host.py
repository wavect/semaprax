"""Linux admission refusals plus explicitly requested physical launcher probe."""
import io
import lzma
import os
import pathlib
import shutil
import sys
import tarfile
import tempfile
import types
import unittest

SUITE = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(SUITE))
from agent import pilot_linux_host as h


def pins():
    return {"schema": h.SCHEMA, "profile": h.PROFILE, "container_path": "/usr/bin/false",
            "container_sha256": "1" * 64, "image": "docker.io/library/rust@sha256:" + "2" * 64,
            "arm64_manifest_sha256": "3" * 64, "kernel_release": "6.18.35", "node_binary_sha256": "4" * 64,
            "launcher_sha256": "5" * 64, "launcher_source_sha256": h.p.digest(h.LAUNCHER_SOURCE.read_bytes()),
            "review_reference": "fixture only; not actual provision admission"}


class LinuxHostTests(unittest.TestCase):
    def test_receipt_is_exact_canonical_explicit_and_reviewed(self):
        value = pins()
        data = h.p.canonical(value)
        self.assertEqual(h.admit_provision(data, h.p.digest(data)), value)
        for key, bad in (("image", "rust:latest"), ("profile", h.p.PROFILE),
                         ("launcher_source_sha256", "0" * 64), ("review_reference", ""),
                         ("container_path", "../container"), ("kernel_release", "current")):
            changed = dict(value, **{key: bad})
            wire = h.p.canonical(changed)
            with self.assertRaises(ValueError, msg=key):
                h.admit_provision(wire, h.p.digest(wire))
        with self.assertRaisesRegex(ValueError, "digest_refused"):
            h.admit_provision(data, "0" * 64)
        with self.assertRaisesRegex(ValueError, "not_canonical"):
            h.admit_provision(data + b" ", h.p.digest(data + b" "))

    def test_archive_selects_only_authenticated_aarch64_binary(self):
        elf = b"\x7fELF\x02\x01" + b"\0" * 10 + b"\x03\0\xb7\0" + b"\0" * 44
        def archive(path, data):
            stream = io.BytesIO()
            with tarfile.open(fileobj=stream, mode="w") as tar:
                info = tarfile.TarInfo(path)
                info.size = len(data)
                tar.addfile(info, io.BytesIO(data))
            return lzma.compress(stream.getvalue())
        wire = archive(h.NODE_ROOT + "/bin/node", elf)
        self.assertEqual(h.linux_node(wire, h.p.digest(elf)), elf)
        with self.assertRaisesRegex(ValueError, "identity_refused"):
            h.linux_node(wire, "0" * 64)
        with self.assertRaisesRegex(ValueError, "member_refused"):
            h.linux_node(archive(h.NODE_ROOT + "/../node", elf), h.p.digest(elf))

    def test_policy_has_no_repository_home_network_or_root_grant(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp).resolve()
            phase = root / "phase"
            phase.mkdir()
            authority = object.__new__(h.LinuxAuthority)
            authority.runtime = types.SimpleNamespace(root=root / "runtime", node=root / "runtime/node")
            authority.pins = pins()
            argv = authority.argv([str(authority.runtime.node), "index.js"], phase, "test")
            self.assertEqual(argv[argv.index("--network") + 1], "none")
            self.assertIn("--read-only", argv)
            self.assertEqual(argv[argv.index("--cap-drop") + 1], "ALL")
            self.assertIn("nproc=64:64", argv)
            self.assertIn("fsize=8388608:8388608", argv)
            mounts = [argv[i + 1] for i, arg in enumerate(argv) if arg == "--mount"]
            self.assertEqual(len(mounts), 2)
            self.assertEqual(argv[-2:], ["/runtime/node", "index.js"])
            self.assertNotIn(str(pathlib.Path.home()), mounts)
            with self.assertRaisesRegex(ValueError, "unbound_command"):
                authority.argv(["/bin/sh"], phase, "test")

    @unittest.skipUnless(os.environ.get("SEMAPRAX_LINUX_LAUNCHER"), "explicit physical launcher provision required")
    def test_physical_landlock_seccomp_denials(self):
        executable = pathlib.Path(os.environ["SEMAPRAX_LINUX_LAUNCHER"]).resolve()
        cli = pathlib.Path(shutil.which("container")).resolve()
        with tempfile.TemporaryDirectory(prefix="pilot-linux-probe-") as tmp:
            root = pathlib.Path(tmp).resolve()
            runtime = root / "runtime"
            runtime.mkdir()
            shutil.copyfile(executable, runtime / "launcher")
            (runtime / "launcher").chmod(0o500)
            provision = pins()
            provision.update(container_path=str(cli), container_sha256=h.p.provenance.file_digest(cli, 128 * 1024 * 1024)[1],
                             image="docker.io/library/rust@sha256:2775a09d208ff0d7c1f50490c45b62db929e87ba1dcbc3f2132ac71a704bcdd3",
                             arm64_manifest_sha256="b28e5606d830400fabf789f910f9ed2ea22cdd6d51d463c5d0baa30bb2bedb2d")
            admitted = types.SimpleNamespace(root=runtime, node=runtime / "node", check=lambda: None)
            authority = h.LinuxAuthority(admitted, provision)
            authority.admit_image()
            observations = authority.preflight(root)
            self.assertEqual(observations["physical_denials"], h.EXPECTED_PROBE)
            self.assertEqual(len(authority.commands), 1)
            self.assertEqual(authority.commands[0]["status"], 0)


    @unittest.skipUnless(os.environ.get("SEMAPRAX_LINUX_PROVISION"), "explicit official Linux provision required")
    def test_official_guest_scores_candidate_and_refuses_bypasses(self):
        directory = pathlib.Path(os.environ["SEMAPRAX_LINUX_PROVISION"])
        expected = os.environ["SEMAPRAX_LINUX_PROVISION_SHA256"]
        plan = {"profile": "typescript-live-pilot.v1", "task_id": h.p.TASK, "candidate_paths": [h.p.CANDIDATE],
                "configuration": {"limits": {"max_result_bytes": 65536}},
                "execution_profiles": {"linux-arm64": {"profile": h.PROFILE, "provision_sha256": expected}}}
        _, _, _, public, _, _ = h.p.source_inputs()
        with h.LinuxCandidateSession(directory, expected_provision_sha256=expected) as session:
            result = session.score_candidate(plan, {h.p.CANDIDATE: public[h.p.CANDIDATE].decode()})
            self.assertEqual(result["status"], "ok", result)
            for source in ("export function validate(k:number,v:number,n:number):number{return 999;}\n",
                           "(globalThis as any).process.exit(0); export function validate(k:number,v:number,n:number):number{return 0;}\n"):
                result = session.score_candidate(plan, {h.p.CANDIDATE: source})
                self.assertEqual(result["status"], "failed")
                self.assertFalse(result["public"]["passed"])
                self.assertFalse(result["hidden"]["passed"])
            evidence = session.evidence()
            self.assertEqual(evidence["result"]["host"]["execution"]["os"], "linux")
            self.assertEqual(len(evidence["result"]["results"]), 3)
            self.assertIn("not_performed", evidence["result"]["model_generation"])
            destination = os.environ.get("SEMAPRAX_LINUX_FIXTURE_EVIDENCE")
            if destination:
                with open(destination, "xb") as output:
                    output.write(h.p.canonical(evidence))


if __name__ == "__main__":
    unittest.main()
