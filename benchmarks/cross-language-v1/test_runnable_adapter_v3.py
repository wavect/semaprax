"""Real official-toolchain and hostile-boundary v3 acceptance tests."""
from __future__ import annotations
import base64
import gzip
import hashlib
import io
import json
import lzma
import os
import pathlib
import struct
import sys
import tarfile
import tempfile
import time
import unittest
from unittest import mock
import uuid

SUITE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(SUITE))
import runnable_adapter_v3 as v3
p = v3.p
x = v3.extraction


def node_tar(entries):
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name, data, kind in entries:
            entry = tarfile.TarInfo(name)
            if kind == "symlink":
                entry.type = tarfile.SYMTYPE
                entry.linkname = "outside"
                archive.addfile(entry)
            else:
                entry.size = len(data)
                archive.addfile(entry, io.BytesIO(data))
    return buffer.getvalue()


class PureProvenanceTests(unittest.TestCase):
    def test_source_manifest_is_complete_and_independently_pinned(self):
        manifest, _ = p.source_snapshot()
        self.assertEqual(p.digest(p.canonical(manifest)), p.SOURCE_HASH)
        self.assertEqual(len(manifest["files"]), 298)
        self.assertEqual(len(manifest["comparison_inventory"]), 182)
        self.assertEqual(len(manifest["task_ids"]), 13)
        blocked = [row for row in manifest["comparison_inventory"] if row["adapter_id"] in ("zero", "ntnt", "aver", "vera", "hale", "moonbit")]
        self.assertEqual(len(blocked), 78)
        self.assertTrue(all(not row["implemented"] and row["blocked_reason"] for row in blocked))

    def test_changed_source_scorer_and_review_are_not_current_hash_authority(self):
        manifest, contents = p.source_snapshot()
        with tempfile.TemporaryDirectory(prefix="r03-source-negative-") as temporary:
            root = pathlib.Path(temporary).resolve()
            for name, data in contents.items():
                destination = root / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(data)
            for name in ("benchmarks/cross-language-v1/run.py", "benchmarks/cross-language-v1/tasks.json",
                         "benchmarks/cross-language-v1/tasks/booking-window-conflict-v1/EQUIVALENCE.md"):
                path = root / name
                original = path.read_bytes()
                path.write_bytes(original + b"\n")
                # Even a caller recomputing its current source digest cannot
                # change the independent expected manifest used by admission.
                self.assertNotEqual(p.digest(path.read_bytes()), p.digest(original))
                with self.assertRaisesRegex(p.Error, "approved_source_input_drifted"):
                    p.source_snapshot(root)
                with mock.patch.object(p, "ROOT", root), mock.patch.object(x, "prepare") as prepare:
                    with self.assertRaisesRegex(p.Error, "approved_source_input_drifted"):
                        with v3.OfficialSession(root):
                            self.fail("drifted source dispatched")
                    prepare.assert_not_called()
                path.write_bytes(original)

    def test_nofollow_rejects_leaf_and_ancestor_substitution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            actual = root / "actual"
            actual.mkdir()
            (actual / "file").write_bytes(b"data")
            (root / "link").symlink_to(actual, target_is_directory=True)
            (root / "leaf").symlink_to(actual / "file")
            for path in (root / "link/file", root / "leaf"):
                with self.assertRaisesRegex(p.Error, "nofollow_acquisition_refused"):
                    p.read_regular(path, 100)

    def test_decoded_exact_limit_and_plus_one_include_skipped_members_and_padding(self):
        raw = node_tar([("node-v22.12.0-darwin-arm64/bin/node", b"node", "file"),
                        ("node-v22.12.0-darwin-arm64/ignored", b"ignored" * 3000, "file")])
        compressed = lzma.compress(raw)
        self.assertEqual(x.scan_archive(compressed, "node", decoded_limit=len(raw)), {"node": b"node"})
        with self.assertRaisesRegex(p.Error, "decoded_stream_exceeds_bound"):
            x.scan_archive(compressed, "node", decoded_limit=len(raw) - 1)
        with self.assertRaisesRegex(p.Error, "decoded_stream_exceeds_bound"):
            x.scan_archive(lzma.compress(raw + b"x"), "node", decoded_limit=len(raw))

    def test_unsafe_duplicate_and_selected_link_members_are_refused(self):
        entry = ("node-v22.12.0-darwin-arm64/bin/node", b"node", "file")
        for entries in ([entry, entry], [entry, ("../outside", b"x", "file")],
                        [(entry[0], b"", "symlink")], [entry, ("/outside", b"x", "file")]):
            with self.assertRaises(p.Error):
                x.scan_archive(lzma.compress(node_tar(entries)), "node")

    def test_extended_metadata_is_bounded_before_tar_parser_allocates_it(self):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
            entry = tarfile.TarInfo("node-v22.12.0-darwin-arm64/bin/node")
            entry.size = 1
            entry.pax_headers = {"comment": "x" * (1024 * 1024 + 1)}
            archive.addfile(entry, io.BytesIO(b"x"))
        with self.assertRaisesRegex(p.Error, "extended_metadata_exceeds_bound"):
            x.scan_archive(lzma.compress(buffer.getvalue()), "node")

    def test_dependency_and_rpath_drift_fail_closed(self):
        def macho(command, name):
            body = struct.pack("<6I", command, 24 + len(name) + 1, 24, 0, 0, 0) + name + b"\0"
            return struct.pack("<8I", 0xfeedfacf, 0x100000c, 0, 2, 1, len(body), 0, 0) + body
        with self.assertRaisesRegex(p.Error, "dependency_identity_drifted"):
            p.check_macho(macho(0xc, b"/attacker/lib.dylib"))
        with self.assertRaisesRegex(p.Error, "rpath_refused"):
            p.check_macho(macho(0x8000001c, b"@loader_path"))

    def test_wrong_receipt_or_archive_refuses_before_extraction(self):
        provision = pathlib.Path(os.environ.get("SPX_R03_V3_PROVENANCE", "/Users/kevin/Documents/ChatGPT/v070-locks/r03-provenance"))
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            for name in ("node22.12.0-SHASUMS256.txt", "typescript5.8.3-registry.json"):
                (root / name).write_bytes(p.read_regular(provision / name, 65536))
            (root / p.NODE_NAME).write_bytes(b"not the node archive")
            (root / p.TS_NAME).write_bytes(b"not the TypeScript archive")
            with self.assertRaisesRegex(p.Error, "node_archive_identity_drifted"):
                p.approved_archives(root)
            (root / p.NODE_NAME).unlink()
            os.link(provision / p.NODE_NAME, root / p.NODE_NAME)
            with self.assertRaisesRegex(p.Error, "typescript_archive_identity_drifted"):
                p.approved_archives(root)
            (root / "typescript5.8.3-registry.json").write_bytes(b"caller invented pin")
            with self.assertRaisesRegex(p.Error, "official_receipt_identity_drifted"):
                p.approved_archives(root)

    def test_exclusive_bounded_evidence_delivery_rejects_overwrite_and_links(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            output = root / "evidence.json"
            v3.deliver(output, {"result": "actual"})
            with self.assertRaises(FileExistsError):
                v3.deliver(output, {"result": "replacement"})
            (root / "linked").symlink_to(output)
            with self.assertRaises(FileExistsError):
                v3.deliver(root / "linked", {})
            with self.assertRaisesRegex(p.Error, "evidence_capacity_exceeded"):
                v3.deliver(root / "oversized", {"x": "x" * v3.MAX_EVIDENCE_BYTES})


class OfficialRuntimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.provision = pathlib.Path(os.environ.get("SPX_R03_V3_PROVENANCE", "/Users/kevin/Documents/ChatGPT/v070-locks/r03-provenance"))
        cls.session = v3.OfficialSession(cls.provision)
        cls.session.__enter__()  # Required real tools: absence fails, never skips.
        cls.counter = 0

    @classmethod
    def tearDownClass(cls):
        try:
            output = cls.provision / ("r03-v3-test-evidence-" + uuid.uuid4().hex + ".json")
            v3.deliver(output, cls.session.evidence())
            print("\nR03_V3_EVIDENCE=" + str(output), flush=True)
        finally:
            cls.session.close()

    def phase(self):
        type(self).counter += 1
        phase = self.session.root / ("negative-phase-" + str(self.counter))
        phase.mkdir(mode=0o700)
        return phase

    def test_authority_controls_are_real_and_nonvacuous(self):
        proof = self.session.observations[0]
        self.assertTrue(proof["listener_positive_connect"])
        self.assertTrue(proof["listener_positive_accept"])
        self.assertEqual(len(proof["denials"]), 5)
        self.assertTrue(all(row.endswith(":EPERM") for row in proof["denials"]))
        self.assertTrue(any(str(self.session.runtime.node) in row for row in proof["denials"]))

    def test_closed_environment_ignores_startup_and_loader_injection(self):
        phase = self.phase()
        code = "console.log(JSON.stringify(process.env))"
        with mock.patch.dict(os.environ, {"NODE_OPTIONS": "--require /attacker.js", "DYLD_INSERT_LIBRARIES": "/attacker.dylib", "PATH": "/attacker"}):
            status, out, _ = self.session.authority.launch([str(self.session.runtime.node), "-e", code], phase, time.monotonic() + 10)
        self.assertEqual(status, 0)
        self.assertEqual(json.loads(out), v3.v1.CLOSED_ENVIRONMENT)

    def test_runtime_file_and_binary_substitution_refuses_before_dispatch(self):
        for path in (self.session.runtime.compiler, self.session.runtime.node):
            prior_count = len(self.session.authority.commands)
            with path.open("rb") as file:
                original = file.read(1)
            mode = path.stat().st_mode & 0o777
            os.chmod(path, 0o600)
            try:
                with path.open("r+b") as file:
                    file.write(bytes([original[0] ^ 1]))
                with self.assertRaisesRegex(p.Error, "runtime_content_identity_drifted"):
                    self.session.authority.launch([str(self.session.runtime.node), "--version"], self.phase(), time.monotonic() + 10)
                self.assertEqual(len(self.session.authority.commands), prior_count)
            finally:
                with path.open("r+b") as file:
                    file.write(original)
                os.chmod(path, mode)
        self.session.runtime.check()

    def test_missing_sandbox_has_no_node_dispatch(self):
        prior_count = len(self.session.authority.commands)
        with mock.patch.object(v3.authority, "SANDBOX", self.session.root / "missing"):
            with self.assertRaises(p.Error):
                v3.authority.Authority(self.session.runtime)
        self.assertEqual(len(self.session.authority.commands), prior_count)

    def test_missing_authenticated_node_has_no_dispatch(self):
        node = self.session.runtime.node
        saved = self.session.runtime.root / "saved-node"
        count = len(self.session.authority.commands)
        os.chmod(self.session.runtime.root, 0o700)
        node.rename(saved)
        try:
            with self.assertRaisesRegex(p.Error, "runtime_inventory_drifted"):
                self.session.authority.launch([str(node), "--version"], self.phase(), time.monotonic() + 10)
            self.assertEqual(len(self.session.authority.commands), count)
        finally:
            saved.rename(node)
            os.chmod(self.session.runtime.root, 0o500)

    def test_output_and_timeout_are_real_bounded_failures(self):
        node = str(self.session.runtime.node)
        phase = self.phase()
        status, _, error = self.session.authority.launch([node, "-e", "process.stdout.write('x'.repeat(1000000))"], phase, time.monotonic() + 10)
        self.assertIsNone(status)
        self.assertEqual(error, "execution_output_exceeded")
        status, _, error = self.session.authority.launch([node, "-e", "setInterval(()=>{},1000)"], phase, time.monotonic() + 0.2)
        self.assertIsNone(status)
        self.assertEqual(error, "execution_timed_out")

    def test_binary_streams_preserve_exact_bytes_in_evidence(self):
        status, out, _ = self.session.authority.launch([str(self.session.runtime.node), "-e", "process.stdout.write(Buffer.from([255,0,65]))"], self.phase(), time.monotonic() + 10)
        self.assertEqual(status, 0)
        self.assertEqual(base64.b64decode(self.session.authority.commands[-1]["stdout_base64"]), b"\xff\0A")
        self.assertNotEqual(out.encode(), b"\xff\0A")

    def test_unbound_command_and_missing_mutant_target_fail_closed(self):
        with self.assertRaisesRegex(p.Error, "unbound_process_command_refused"):
            self.session.authority.launch(["/usr/bin/true"], self.phase(), time.monotonic() + 10)
        phase = self.phase()
        (phase / "candidate.ts").write_text("export const unrelated = 1;")
        with self.assertRaisesRegex(p.Error, "mutant_target_missing_or_ambiguous"):
            self.session._mutate(phase, self.session.mutants["booking-window-conflict-v1"])

    def test_malformed_scoring_result_and_fake_pass_without_dispatch_are_refused(self):
        for row in ({"passed": "true", "phase": "run", "detail": ["invented"]},
                    {"passed": True, "phase": "run", "detail": ["invented"]},
                    {"passed": False, "phase": "unknown", "detail": []}):
            with mock.patch.object(self.session.scorer, "stage", return_value=row):
                with self.assertRaisesRegex(p.Error, "scoring_result_shape_or_dispatch_refused"):
                    self.session.score("booking-window-conflict-v1")

    def test_compile_failure_cannot_satisfy_runtime_mutant_gate(self):
        row = self.session.mutants["booking-window-conflict-v1"]
        altered = dict(row, replacement="INVALID TYPESCRIPT SYNTAX")
        with mock.patch.dict(self.session.mutants, {"booking-window-conflict-v1": altered}):
            with self.assertRaisesRegex(p.Error, "mutant_did_not_prove_expected_runtime_divergence"):
                self.session.score("booking-window-conflict-v1", mutant=True)


def positive(task):
    def test(self):
        row = self.session.score(task)
        self.assertEqual(row["status"], "ok", row)
        self.assertTrue(row["public"]["passed"])
        self.assertTrue(row["hidden"]["passed"])
        self.assertEqual(row["leak_check"], "ok")
    return test


def mutant(task):
    def test(self):
        row = self.session.score(task, mutant=True)
        self.assertEqual(row["status"], "failed", row)
        self.assertFalse(row["hidden"]["passed"])
        self.assertEqual(row["hidden"]["phase"], "run")
        self.assertEqual(row["mutation"]["target_count"], 1)
        self.assertNotEqual(row["mutation"]["before_sha256"], row["mutation"]["after_sha256"])
        self.assertEqual(row["leak_check"], "ok")
    return test


for task in json.loads(p.read_regular(p.SOURCE_MANIFEST, 128 * 1024))["task_ids"]:
    setattr(OfficialRuntimeTests, "test_positive_" + task.replace("-", "_"), positive(task))
    setattr(OfficialRuntimeTests, "test_mutant_" + task.replace("-", "_"), mutant(task))

if __name__ == "__main__":
    unittest.main()
