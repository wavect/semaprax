#!/usr/bin/env python3
"""One isolated, evidence-first OpenCode tuple transport (no matrix scheduler)."""

import argparse
import hashlib
import importlib.util
import json
import os
import pwd
import selectors
import shutil
import signal
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODEL = "opencode/muse-spark-1.3-contributor-free"
AGENT = "semaprax-pilot"
CAP = 1_048_576


class PilotFailure(Exception):
    pass


def load_runner():
    p = ROOT / "scripts/agent-task-comparison-runner.py"
    s = importlib.util.spec_from_file_location("atc_runner", p)
    m = importlib.util.module_from_spec(s)
    s.loader.exec_module(m)
    return m


runner = load_runner()


def sha(b):
    return hashlib.sha256(b).hexdigest()


def snapshot(root):
    return runner.snapshot_candidate(root)


def inside(parent, child):
    try:
        child.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def policy():
    # OpenCode documented v1 permission keys; seatbelt remains the OS boundary.
    return {
        "$schema": "https://opencode.ai/config.json",
        "model": MODEL,
        "agent": {
            AGENT: {
                "mode": "primary",
                "model": MODEL,
                "permission": {
                    "*": "deny",
                    "read": "allow",
                    "glob": "allow",
                    "grep": "allow",
                    "list": "allow",
                    "edit": "allow",
                    "bash": {"*": "deny", "semaprax *": "allow"},
                    "webfetch": "deny",
                    "websearch": "deny",
                    "task": "deny",
                    "external_directory": "deny",
                },
            }
        },
    }


def bounded(argv, cwd, timeout, env=None, check=True):
    try:
        p = subprocess.Popen(
            argv,
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
    except OSError as e:
        raise PilotFailure(f"spawn failed: {e}") from e
    streams = {p.stdout: bytearray(), p.stderr: bytearray()}
    sel = selectors.DefaultSelector()
    for q in streams:
        sel.register(q, selectors.EVENT_READ)
    deadline = time.monotonic() + timeout
    failure = None
    try:
        while sel.get_map():
            left = deadline - time.monotonic()
            if left <= 0:
                failure = "subprocess timed out"
                break
            for key, _ in sel.select(min(left, 0.02)):
                chunk = os.read(key.fileobj.fileno(), 8192)
                if not chunk:
                    sel.unregister(key.fileobj)
                    continue
                streams[key.fileobj].extend(chunk)
                if sum(map(len, streams.values())) > CAP:
                    failure = "subprocess output cap exceeded"
                    break
            # The direct command may exit while a descendant retains either
            # pipe.  Reap the whole fresh process group immediately; waiting
            # for EOF first would let that descendant outlive the command.
            if p.poll() is not None:
                try:
                    os.killpg(p.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            if failure:
                break
    finally:
        sel.close()
        if failure:
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            p.wait()
        else:
            try:
                p.wait(timeout=max(0, deadline - time.monotonic()))
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(p.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                p.wait()
                failure = "subprocess timed out"
            else:
                try:
                    os.killpg(p.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
    out, err = bytes(streams[p.stdout]), bytes(streams[p.stderr])
    p.stdout.close()
    p.stderr.close()
    if failure:
        raise PilotFailure(failure)
    if check and p.returncode:
        raise PilotFailure(f"subprocess exited {p.returncode}: {err[:300]!r}")
    return out, err, p.returncode


def original_repository_root():
    git = shutil.which("git")
    if not git:
        raise PilotFailure("git unavailable for original-repository discovery")
    out, _, _ = bounded(
        [git, "rev-parse", "--path-format=absolute", "--git-common-dir"], ROOT, 5
    )
    common = Path(out.decode().strip()).resolve()
    if common.name != ".git" or not common.is_dir():
        raise PilotFailure("cannot establish original repository root")
    return common.parent


def seatbelt_profile(candidate, protected_roots, state):
    # Physical paths close macOS /var -> /private/var aliases.
    candidate = Path(candidate).resolve(strict=True)
    state = Path(state).resolve(strict=True)
    roots = sorted({Path(root).resolve(strict=True) for root in protected_roots})
    if any(inside(root, candidate) or inside(root, state) for root in roots):
        raise PilotFailure("candidate or host state overlaps a protected root")
    read_rules = "".join(
        f"(deny file-read* (require-any (literal {json.dumps(str(root))}) (subpath {json.dumps(str(root))})))\n"
        for root in roots
    )
    writable = " ".join(
        f"(subpath {json.dumps(str(path))})" for path in (candidate, state)
    )
    return (
        "(version 1)\n(allow default)\n"
        + read_rules
        + f"(deny file-write* (require-not (require-any {writable})))\n"
    )


def seatbelt_probe(profile, candidate_file, protected_files, candidate, state):
    """Probe the saved production profile, writing only to owned temp locations."""
    profile = Path(profile).resolve(strict=True)
    candidate = Path(candidate).resolve(strict=True)
    state = Path(state).resolve(strict=True)
    candidate_file = Path(candidate_file).resolve(strict=True)
    if not inside(candidate, candidate_file):
        raise PilotFailure("probe read file is outside candidate")
    with tempfile.TemporaryDirectory(prefix="spx-seatbelt-probe-") as temp:
        outside = Path(temp).resolve() / "outside"
        outside.write_text("unchanged")
        candidate_write = candidate / ".seatbelt-probe"
        state_write = state / ".seatbelt-probe"
        read_link = candidate / ".seatbelt-read-link"
        write_link = candidate / ".seatbelt-write-link"
        if any(
            path.exists() or path.is_symlink()
            for path in (candidate_write, state_write, read_link, write_link)
        ):
            raise PilotFailure("probe output already exists")

        def invoke(executable, *args):
            return bounded(
                sandboxed(executable, profile, list(args)), state, 5, check=False
            )[2]

        def write(path):
            return invoke("/bin/sh", "-c", 'printf probe > "$1"', "sh", str(path))

        try:
            if invoke("/bin/cat", str(candidate_file)):
                raise PilotFailure("sandbox cannot read candidate fixture")
            if write(candidate_write) or write(state_write):
                raise PilotFailure(
                    "sandbox cannot write candidate or private host state"
                )
            for protected_file in protected_files:
                protected_file = Path(protected_file).resolve(strict=True)
                if not protected_file.is_file() or not os.access(
                    protected_file, os.R_OK
                ):
                    raise PilotFailure("protected probe file is not host-readable")
                if not invoke("/bin/cat", str(protected_file)):
                    raise PilotFailure(
                        f"sandbox read protected descendant: {protected_file}"
                    )
                read_link.symlink_to(protected_file)
                try:
                    if not invoke("/bin/cat", str(read_link)):
                        raise PilotFailure(
                            f"sandbox read protected descendant through candidate symlink: {protected_file}"
                        )
                finally:
                    read_link.unlink()
            if not write(outside) or outside.read_text() != "unchanged":
                raise PilotFailure(
                    "sandbox wrote outside candidate and private host state"
                )
            write_link.symlink_to(outside)
            if not write(write_link) or outside.read_text() != "unchanged":
                raise PilotFailure("sandbox wrote outside through candidate symlink")
        finally:
            for path in (candidate_write, state_write, read_link, write_link):
                path.unlink(missing_ok=True)


def sandboxed(opencode, profile, args):
    exe = shutil.which("sandbox-exec")
    if not exe:
        raise PilotFailure("sandbox-exec unavailable")
    return [exe, "-f", str(profile), str(opencode), *args]


def provision_semaprax(source, state):
    source = Path(source)
    if not source.is_absolute() or source.is_symlink():
        raise PilotFailure("--semaprax must be an absolute non-symlink executable")
    try:
        stat = source.stat()
    except OSError as error:
        raise PilotFailure("--semaprax is not accessible") from error
    if not stat.st_mode & 0o111 or not source.is_file():
        raise PilotFailure("--semaprax must be a regular executable")

    def file_sha(path):
        digest = hashlib.sha256()
        with Path(path).open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        return digest.hexdigest()

    before = file_sha(source)
    destination = Path(state) / "bin" / "semaprax"
    destination.parent.mkdir()
    shutil.copyfile(source, destination)
    destination.chmod(stat.st_mode & 0o777)
    after_source = file_sha(source)
    after = file_sha(destination)
    if before != after_source or before != after:
        raise PilotFailure("--semaprax copy digest changed")
    return destination, after


def private_environment(state, config, compiler_bin):
    """Keep inherited credentials and user home out of the model process."""
    paths = {
        "HOME": state / "home",
        "XDG_CONFIG_HOME": state / "config",
        "XDG_DATA_HOME": state / "data",
        "XDG_CACHE_HOME": state / "cache",
        "TMPDIR": state / "tmp",
    }
    for path in paths.values():
        path.mkdir()
    return {
        "PATH": f"{Path(compiler_bin).resolve(strict=True)}:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
        "LANG": "en_US.UTF-8",
        "NO_COLOR": "1",
        **{key: str(value) for key, value in paths.items()},
        "OPENCODE_CONFIG": str(config.resolve(strict=True)),
        "OPENCODE_CONFIG_DIR": str(paths["XDG_CONFIG_HOME"]),
    }


def run_tuple(task, lane, trial, opencode, semaprax, evidence, timeout=600):
    if lane != "semaprax-source-first":
        raise PilotFailure("this transport currently supports source-first only")
    if trial < 1 or timeout <= 0:
        raise PilotFailure("trial and timeout must be positive")
    evidence = Path(evidence)
    if not evidence.parent.is_dir() or evidence.exists():
        raise PilotFailure(
            "evidence destination must be a new child of an existing directory"
        )
    evidence = evidence.parent.resolve(strict=True) / evidence.name
    _, _, tasks = runner.atc.load_manifest(
        "benchmarks/agent-task-comparison-v1/manifest.json"
    )
    binding = next((x for x in tasks if x["id"] == task), None)
    if binding is None:
        raise PilotFailure(f"unknown task: {task}")
    prompt = json.loads((ROOT / binding["path"]).read_text(encoding="utf-8"))["prompt"]
    original = original_repository_root()
    sandbox, candidate = runner.create_sandbox(binding)
    before = snapshot(candidate)
    try:
        with tempfile.TemporaryDirectory(prefix="spx-opencode-pilot-") as temp:
            host = Path(temp).resolve()
            config = host / "opencode.json"
            config.write_text(json.dumps(policy(), sort_keys=True))
            user_home = Path(pwd.getpwuid(os.getuid()).pw_dir).resolve(strict=True)
            profile = host / "seatbelt.sb"
            profile.write_text(
                seatbelt_profile(
                    candidate, (ROOT, original, evidence.parent, user_home), host
                )
            )
            protected_files = {
                ROOT / "benchmarks/agent-task-comparison-v1/manifest.json",
                original / "benchmarks/agent-task-comparison-v1/manifest.json",
            }
            seatbelt_probe(
                profile, candidate / "semaprax.toml", protected_files, candidate, host
            )
            compiler, compiler_digest = provision_semaprax(semaprax, host)
            env = private_environment(host, config, compiler.parent)
            start = time.monotonic_ns()
            out, err, _ = bounded(
                sandboxed(
                    opencode,
                    profile,
                    [
                        "run",
                        "--pure",
                        "--agent",
                        AGENT,
                        "--model",
                        MODEL,
                        "--format",
                        "json",
                        "--dir",
                        str(candidate),
                        prompt,
                    ],
                ),
                host,
                timeout,
                env,
            )
            elapsed = time.monotonic_ns() - start
            try:
                session = next(
                    (
                        json.loads(x).get("sessionID")
                        for x in out.decode().splitlines()
                        if x
                    ),
                    None,
                )
            except (UnicodeError, json.JSONDecodeError) as error:
                raise PilotFailure("raw stream is not valid JSONL") from error
            if not isinstance(session, str) or not session:
                raise PilotFailure("raw stream lacks session id")
            exported, _, _ = bounded(
                sandboxed(opencode, profile, ["export", session, "--pure"]),
                host,
                timeout,
                env,
            )
            after = snapshot(candidate)
            evidence.mkdir()
            (evidence / "stdout.jsonl").write_bytes(out)
            (evidence / "stderr.txt").write_bytes(err)
            (evidence / "session.json").write_bytes(exported)
            (evidence / "seatbelt.sb").write_bytes(profile.read_bytes())
            record = {
                "schema": "semaprax.opencode-agent-task-pilot.v1",
                "status": "ineligible",
                "reason": "blinded review and ledger metric mapping are not implemented",
                "task": task,
                "lane": lane,
                "trial": trial,
                "model": MODEL,
                "semaprax_sha256": compiler_digest,
                "session_id": session,
                "wall_ns": elapsed,
                "before": before,
                "after": after,
                "stdout_sha256": sha(out),
                "session_sha256": sha(exported),
            }
            (evidence / "record.json").write_text(
                json.dumps(record, sort_keys=True) + "\n"
            )
            return record
    finally:
        shutil.rmtree(sandbox, ignore_errors=True)


def main():
    a = argparse.ArgumentParser()
    a.add_argument("--task", required=True)
    a.add_argument("--lane", required=True)
    a.add_argument("--trial", type=int, required=True)
    a.add_argument("--opencode", default="/opt/homebrew/bin/opencode")
    a.add_argument(
        "--semaprax",
        required=True,
        help="absolute compiler executable to copy into private host state",
    )
    a.add_argument("--evidence-dir", required=True)
    a.add_argument("--timeout", type=int, default=600)
    n = a.parse_args()
    print(
        json.dumps(
            run_tuple(
                n.task,
                n.lane,
                n.trial,
                n.opencode,
                n.semaprax,
                Path(n.evidence_dir),
                n.timeout,
            ),
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
