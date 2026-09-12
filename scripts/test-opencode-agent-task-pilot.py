#!/usr/bin/env python3
import importlib.util
import json
import os
import shutil
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location(
    "pilot", ROOT / "scripts/opencode-agent-task-pilot.py"
)
pilot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pilot)


class PilotTests(unittest.TestCase):
    def test_policy_denies_network_subagents_and_external_paths(self):
        config = pilot.policy()
        self.assertEqual(config["model"], pilot.MODEL)
        self.assertEqual(config["agent"][pilot.AGENT]["model"], pilot.MODEL)
        permissions = config["agent"][pilot.AGENT]["permission"]
        self.assertEqual(permissions["webfetch"], "deny")
        self.assertEqual(permissions["task"], "deny")
        self.assertEqual(permissions["external_directory"], "deny")

    def test_source_first_only(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(pilot.PilotFailure):
                pilot.run_tuple(
                    "signature-migration-v1",
                    "semaprax-graph-operational",
                    1,
                    "unused",
                    str(Path(sys.executable).resolve(strict=True)),
                    Path(temp) / "new",
                )

    def test_new_evidence_required(self):
        with tempfile.TemporaryDirectory() as temp:
            with self.assertRaises(pilot.PilotFailure):
                pilot.run_tuple(
                    "signature-migration-v1",
                    "semaprax-source-first",
                    1,
                    "unused",
                    str(Path(sys.executable).resolve(strict=True)),
                    Path(temp),
                )


class SubprocessBoundaryTests(unittest.TestCase):
    def test_output_cap_kills_continuous_writer(self):
        old = pilot.CAP
        pilot.CAP = 1024
        try:
            with self.assertRaisesRegex(pilot.PilotFailure, "output cap"):
                pilot.bounded(
                    [
                        sys.executable,
                        "-c",
                        "import sys\nwhile True: sys.stdout.write('x'*512);sys.stdout.flush()",
                    ],
                    ROOT,
                    2,
                )
        finally:
            pilot.CAP = old

    def test_closed_pipes_still_obey_timeout(self):
        with self.assertRaisesRegex(pilot.PilotFailure, "timed out"):
            pilot.bounded(
                [
                    sys.executable,
                    "-c",
                    "import os,time;os.close(1);os.close(2);time.sleep(2)",
                ],
                ROOT,
                0.02,
            )

    def test_exited_parent_cannot_leave_quiet_pipe_descendant(self):
        with tempfile.TemporaryDirectory(prefix="spx-descendant-test-") as temp:
            pid_file = Path(temp) / "pid"
            child_code = f"import os, pathlib, time; pathlib.Path({str(pid_file)!r}).write_text(str(os.getpid())); time.sleep(5)"
            code = f"import subprocess, sys, time; subprocess.Popen([sys.executable, '-c', {child_code!r}]); time.sleep(0.1); print('parent done')"
            start = time.monotonic()
            pilot.bounded([sys.executable, "-c", code], ROOT, 2)
            self.assertLess(time.monotonic() - start, 1)
            for _ in range(20):
                if pid_file.exists():
                    break
                time.sleep(0.01)
            self.assertTrue(pid_file.exists())
            pid = int(pid_file.read_text())
            for _ in range(50):
                try:
                    os.kill(pid, 0)
                except ProcessLookupError:
                    break
                time.sleep(0.01)
            else:
                self.fail("quiet-pipe descendant survived process-group cleanup")

    def test_compiler_source_must_be_regular_non_symlink_executable(self):
        with tempfile.TemporaryDirectory(prefix="spx-compiler-test-") as temp:
            root = Path(temp)
            nonexec = root / "nonexec"
            nonexec.write_text("compiler")
            with self.assertRaisesRegex(pilot.PilotFailure, "regular executable"):
                pilot.provision_semaprax(nonexec, root / "state")
            link = root / "link"
            link.symlink_to(Path(sys.executable).resolve(strict=True))
            with self.assertRaisesRegex(pilot.PilotFailure, "non-symlink"):
                pilot.provision_semaprax(link, root / "state")

    @unittest.skipUnless(shutil.which("sandbox-exec"), "sandbox-exec unavailable")
    def test_saved_profile_denies_directory_descendants_and_outside_write(self):
        with tempfile.TemporaryDirectory(prefix="spx-profile-test-") as temp:
            root = Path(temp).resolve()
            candidate = root / "candidate"
            protected = root / "protected"
            state = root / "state"
            for directory in (candidate, protected, state):
                directory.mkdir()
            allowed = candidate / "fixture.txt"
            hidden = protected / "nested" / "rubric.json"
            hidden.parent.mkdir()
            allowed.write_text("candidate")
            hidden.write_text("rubric")
            profile = state / "seatbelt.sb"
            profile.write_text(pilot.seatbelt_profile(candidate, (protected,), state))
            pilot.seatbelt_probe(profile, allowed, (hidden,), candidate, state)
            self.assertFalse((candidate / ".seatbelt-probe").exists())
            self.assertFalse((state / ".seatbelt-probe").exists())

    @unittest.skipUnless(shutil.which("sandbox-exec"), "sandbox-exec unavailable")
    def test_probe_rejects_the_old_directory_literal_rule(self):
        with tempfile.TemporaryDirectory(prefix="spx-old-profile-test-") as temp:
            root = Path(temp).resolve()
            candidate = root / "candidate"
            protected = root / "protected"
            state = root / "state"
            for directory in (candidate, protected, state):
                directory.mkdir()
            allowed = candidate / "fixture.txt"
            hidden = protected / "rubric.json"
            allowed.write_text("candidate")
            hidden.write_text("rubric")
            profile = state / "old-seatbelt.sb"
            profile.write_text(
                "(version 1)\n(allow default)\n"
                f'(deny file-read* (literal "{protected}"))\n'
                f'(deny file-write* (require-not (require-any (subpath "{candidate}") (subpath "{state}"))))\n'
            )
            with self.assertRaisesRegex(
                pilot.PilotFailure, "read protected descendant"
            ):
                pilot.seatbelt_probe(profile, allowed, (hidden,), candidate, state)


class TupleTransportTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which("sandbox-exec"), "sandbox-exec unavailable")
    def test_real_wrapped_run_and_export_archive_their_boundary_proof(self):
        with tempfile.TemporaryDirectory(prefix="spx-stub-test-") as temp:
            root = Path(temp).resolve()
            evidence_parent = root / "evidence-parent"
            evidence_parent.mkdir()
            evidence = evidence_parent / "run"
            hidden = evidence_parent / "hidden-rubric.json"
            hidden.write_text("rubric")
            outside = root / "outside"
            outside.write_text("unchanged")
            originals = [
                ROOT / "benchmarks/agent-task-comparison-v1/manifest.json",
                pilot.original_repository_root()
                / "benchmarks/agent-task-comparison-v1/manifest.json",
                hidden,
            ]
            stub = root / "opencode-stub"
            stub.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, shutil, subprocess, sys\n"
                "from pathlib import Path\n"
                f"protected={list(map(str, originals))!r}\n"
                f"outside=Path({str(outside)!r})\n"
                "args=sys.argv[1:]\n"
                "phase=args[0]\n"
                "assert phase in ('run','export')\n"
                "assert '--pure' in args\n"
                "assert 'PILOT_TEST_SECRET' not in os.environ\n"
                "config=Path(os.environ['OPENCODE_CONFIG'])\n"
                "assert config.is_absolute() and config.is_file()\n"
                "assert json.loads(config.read_text())['agent']\n"
                "assert Path(os.environ['HOME']).parent == config.parent\n"
                "assert all(Path(os.environ[x]).parent == config.parent for x in ('XDG_CONFIG_HOME','XDG_DATA_HOME','XDG_CACHE_HOME','TMPDIR'))\n"
                "compiler=shutil.which('semaprax')\n"
                "assert compiler and Path(compiler).parent == config.parent/'bin'\n"
                "assert subprocess.run(['semaprax','--version'], stdout=subprocess.PIPE, stderr=subprocess.PIPE).returncode == 0\n"
                "for item in protected:\n"
                "    try: Path(item).read_bytes()\n"
                "    except OSError: pass\n"
                "    else: raise SystemExit('protected read succeeded: '+item)\n"
                "try: outside.write_text('escaped')\n"
                "except OSError: pass\n"
                "else: raise SystemExit('outside write succeeded')\n"
                "if phase == 'run':\n"
                f"    assert args[args.index('--model')+1] == {pilot.MODEL!r}\n"
                "    candidate=Path(args[args.index('--dir')+1])\n"
                "    assert (candidate/'semaprax.toml').is_file()\n"
                "    (candidate/'agent-proof').write_text('run was confined')\n"
                "    print(json.dumps({'sessionID':'stub-session','boundary':'enforced'}))\n"
                "else:\n"
                "    assert args[1] == 'stub-session'\n"
                "    (config.parent/'export-proof').write_text('export was confined')\n"
                "    print(json.dumps({'phase':'export','boundary':'enforced'}))\n"
            )
            stub.chmod(0o700)
            with mock.patch.dict(os.environ, {"PILOT_TEST_SECRET": "must-not-pass"}):
                compiler_source = Path(sys.executable).resolve(strict=True)
                compiler_before = compiler_source.read_bytes()
                record = pilot.run_tuple(
                    "signature-migration-v1",
                    "semaprax-source-first",
                    1,
                    stub,
                    str(compiler_source),
                    evidence,
                    10,
                )
            self.assertEqual(compiler_source.read_bytes(), compiler_before)
            self.assertEqual(record["semaprax_sha256"], pilot.sha(compiler_before))
            self.assertEqual(record["status"], "ineligible")
            self.assertEqual(outside.read_text(), "unchanged")
            self.assertEqual(
                json.loads((evidence / "stdout.jsonl").read_text())["boundary"],
                "enforced",
            )
            self.assertEqual(
                json.loads((evidence / "session.json").read_text()),
                {"phase": "export", "boundary": "enforced"},
            )
            self.assertIn("agent-proof", record["after"])
            self.assertNotIn("agent-proof", record["before"])
            self.assertIn("subpath", (evidence / "seatbelt.sb").read_text())
            self.assertEqual(
                record["stdout_sha256"],
                pilot.sha((evidence / "stdout.jsonl").read_bytes()),
            )


if __name__ == "__main__":
    unittest.main()
