#!/usr/bin/env python3
"""One isolated, evidence-first OpenCode tuple transport (no matrix scheduler)."""

import argparse
import base64
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

from opencode_agent_task_pilot.artifacts import archive_candidate, collect_source_bytes
from opencode_agent_task_pilot.eligibility import (
    INTERVENTION_KINDS,
    append_intervention,
    compute_eligibility,
    initialize_intervention_ledger,
    record_blinded_review,
    finish_blinded_review,
    start_blinded_review,
    write_presentation_evidence,
)
from opencode_agent_task_pilot.evidence import gateway_diagnostics, mcp_tool_metrics, provider_usage
from opencode_agent_task_pilot.review_workflow import audit_cohort, load_frozen_protocol, prepare_review_packet
from opencode_agent_task_pilot.review_workflow import (
    _read_regular, FROZEN_MANIFEST_SHA256, FROZEN_TASK_SHA256, FROZEN_FIXTURE_SHA256,
)

ROOT = Path(__file__).resolve().parent.parent
MODEL = "opencode/muse-spark-1.3-contributor-free"
AGENT = "semaprax-pilot"
CAP = 1_048_576


class PilotFailure(Exception):
    def __init__(self, message, stdout=b"", stderr=b""):
        super().__init__(message)
        self.stdout = stdout
        self.stderr = stderr


def load_runner():
    p = ROOT / "scripts/agent-task-comparison-runner.py"
    s = importlib.util.spec_from_file_location("atc_runner", p)
    m = importlib.util.module_from_spec(s)
    s.loader.exec_module(m)
    return m


runner = load_runner()


def sha(b):
    return hashlib.sha256(b).hexdigest()


def exclusive_write(path, body, limit=8 * CAP):
    if len(body) > limit:
        raise PilotFailure("evidence output exceeds bound")
    fd = None
    created = False
    try:
        if not hasattr(os, "O_NOFOLLOW"):
            raise PilotFailure("safe no-follow evidence writes are unavailable")
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        created = True
        written = 0
        while written < len(body):
            count = os.write(fd, body[written:])
            if count <= 0:
                raise PilotFailure("evidence output made no progress")
            written += count
    except (OSError, PilotFailure) as error:
        if created:
            try:
                held = os.fstat(fd)
                current = os.stat(path, follow_symlinks=False)
                if (held.st_dev, held.st_ino) == (current.st_dev, current.st_ino):
                    os.unlink(path)
            except OSError:
                pass
        raise PilotFailure("evidence output could not be created exclusively") from error
    finally:
        if fd is not None:
            os.close(fd)


def frozen_input_binding(binding):
    manifest = ROOT / "benchmarks/agent-task-comparison-v1/manifest.json"
    manifest_bytes = _read_regular(manifest, 256 * 1024)
    if sha(manifest_bytes) != FROZEN_MANIFEST_SHA256:
        raise PilotFailure("frozen pilot manifest changed")
    task_path = (ROOT / binding["path"]).resolve()
    task_bytes = _read_regular(task_path, 256 * 1024)
    fixture = runner.fixture_root_for(binding)
    inventory = collect_source_bytes(fixture)
    inventory_json = json.dumps(
        {name: sha(body) for name, body in sorted(inventory.items())},
        sort_keys=True, separators=(",", ":"),
    ).encode()
    if sha(task_bytes) != FROZEN_TASK_SHA256.get(binding["id"]):
        raise PilotFailure("frozen pilot task changed")
    if sha(inventory_json) != FROZEN_FIXTURE_SHA256.get(binding["id"]):
        raise PilotFailure("frozen pilot fixture changed")
    return sha(manifest_bytes), sha(task_bytes), sha(inventory_json), inventory, task_bytes


def snapshot(root):
    return runner.snapshot_candidate(root)


def inside(parent, child):
    try:
        child.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def policy(lane="semaprax-source-first", mcp=None, model=MODEL):
    # OpenCode documented v1 permission keys; seatbelt remains the OS boundary.
    if lane not in ("semaprax-source-first", "semaprax-graph-operational"):
        raise PilotFailure("pilot lane is not available")
    graph_lane = lane == "semaprax-graph-operational"
    permissions = {
        "*": "deny",
        "bash": {"*": "deny"},
        "webfetch": "deny",
        "websearch": "deny",
        "task": "deny",
        "external_directory": "deny",
    }
    if graph_lane:
        # The graph lane has no ambient raw source surface. Its gateway logs
        # bounded semantic inspection, proposal materialization and apply
        # commands; the only source mutation is a compiler patch/apply call.
        permissions.update({key: "deny" for key in ("read", "glob", "grep", "list", "edit")})
    else:
        # Source-first remains conventional canonical source work, but its
        # reads and writes go through the same argv MCP transport so the
        # stale trigger and byte counters are evidence rather than guesses.
        permissions.update({key: "deny" for key in ("read", "glob", "grep", "list", "edit")})
    permissions["semaprax_*"] = "allow"
    config = {
        "$schema": "https://opencode.ai/config.json",
        "model": model,
        "agent": {
            AGENT: {
                "mode": "primary",
                "model": model,
                "permission": permissions,
            }
        },
    }
    if mcp is not None:
        config["mcp"] = mcp
    return config


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
        raise PilotFailure(failure, out, err)
    if check and p.returncode:
        raise PilotFailure(f"subprocess exited {p.returncode}: {err[:300]!r}", out, err)
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
        + f'(deny file-write* (require-not (require-any {writable} (literal "/dev/null"))))\n'
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


def prepare_drift(binding, candidate, state):
    """Bind the manifest-authenticated stale edit for the gateway trigger."""
    declared = binding["drift_patch"]
    if declared is None:
        return None
    patch = (ROOT / declared["path"]).read_bytes()
    if runner.digest(patch) != declared["sha256"]:
        raise PilotFailure("drift patch bytes disagree with the manifest binding")
    target = Path(candidate) / "src" / "core.spx"
    before = target.read_bytes()
    after = runner.apply_unified_diff_single_hunk(before, patch, "src/core.spx")
    if before == after:
        raise PilotFailure("manifest drift patch does not change the candidate")
    payload = {
        "target": "src/core.spx",
        "before_b64": base64.b64encode(before).decode("ascii"),
        "after_b64": base64.b64encode(after).decode("ascii"),
        "before_sha256": sha(before),
        "after_sha256": sha(after),
        "applied": False,
    }
    path = Path(state) / "drift.json"
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")
    return path


def install_gateway(compiler, state, candidate, lane, drift=None):
    """Install a recorded, lane-limited compiler command gateway."""
    compiler = Path(compiler).resolve(strict=True)
    state = Path(state).resolve(strict=True)
    candidate = Path(candidate).resolve(strict=True)
    # This function is destructive: it RENAMES `compiler` to `semaprax-real`
    # and writes the wrapper over that exact path. Handed a path outside the
    # disposable state directory it will happily eat a real system binary --
    # passing `sys.executable` renamed this machine's Python 3.14 to
    # `semaprax-real` twice (2026-09-13 and 2026-09-17), leaving every
    # `#!/usr/bin/env python3` shebang recursing until E2BIG. `provision_semaprax`
    # already copies the compiler to `<state>/bin/semaprax`, so every legitimate
    # caller is inside `state`; refuse anything else before touching the disk.
    if not compiler.is_relative_to(state):
        raise PilotFailure(
            f"install_gateway refuses to rename {compiler}: the compiler must live "
            f"inside the disposable state directory {state}. Pass the copy returned "
            f"by provision_semaprax(), never a real interpreter or installed binary."
        )
    real = compiler.with_name("semaprax-real")
    compiler.replace(real)
    log = state / "gateway.jsonl"
    config = json.dumps(
        {"candidate": str(candidate), "lane": lane, "log": str(log), "real": str(real),
         "drift": None if drift is None else str(Path(drift).resolve(strict=True))},
        sort_keys=True,
    )
    wrapper = r'''#!/usr/bin/env python3
import base64, hashlib, json, os, selectors, signal, subprocess, sys, time
from pathlib import Path

cfg = json.loads(os.environ["SEMAPRAX_PILOT_GATEWAY"])
candidate = Path(cfg["candidate"]).resolve()
CAP = 1048576
LOG_CAP = 32 * CAP
source = {"pilot-read", "pilot-write-source", "check", "fmt", "run", "test", "--version"}
graph = {
    "graph", "context", "query", "impact", "review", "patch-evidence",
    "patch-with-evidence", "workspace-init", "semantic-workspace-init",
    "semantic-workspace-change-preview", "semantic-workspace-change-evidence",
    "verify-semantic-workspace-change-evidence",
    "apply-semantic-workspace-change-evidence",
    "semantic-workspace-structural-change-preview",
    "semantic-workspace-operations-derive", "workspace-snapshot",
    "workspace-graph", "workspace-context", "workspace-impact",
    "workspace-review", "workspace-preview", "workspace-apply",
    "workspace-patch-evidence", "verify-workspace-patch-evidence",
    "workspace-apply-with-evidence", "patch", "check", "run", "test",
    "--version", "pilot-write",
}
def record(argv, output, error, code):
    event = {"argv_b64": base64.b64encode("\0".join(argv).encode()).decode(),
        "stdout_b64": base64.b64encode(output).decode(),
        "stderr_b64": base64.b64encode(error).decode(), "returncode": code}
    body = json.dumps(event, sort_keys=True).encode() + b"\n"
    log = Path(cfg["log"])
    if (log.stat().st_size if log.exists() else 0) + len(body) > LOG_CAP:
        raise ValueError("gateway log exceeds cap")
    with open(log, "ab") as stream: stream.write(body)
def denied(message):
    error = (message + "\n").encode()
    try: record(sys.argv[1:], b"", error, 126)
    except ValueError: pass
    sys.stderr.buffer.write(error)
    raise SystemExit(126)
def reserve_log():
    log = Path(cfg["log"])
    if (log.stat().st_size if log.exists() else 0) + 4 * CAP > LOG_CAP:
        denied("gateway log capacity exhausted before dispatch")
def apply_drift(args):
    drift_path = cfg.get("drift")
    identifying = {"graph", "context", "query", "workspace-graph", "workspace-context"}
    command = args[0]
    source_read = False
    if command == "pilot-read" and len(args) == 2:
        try:
            source_read = (candidate / Path(args[1])).resolve() == candidate / "src" / "core.spx"
        except OSError:
            source_read = False
    if not drift_path or (command not in identifying and not source_read):
        return
    value = json.loads(Path(drift_path).read_text())
    if value.get("applied"):
        return
    if value.get("target") != "src/core.spx":
        denied("invalid drift target")
    target = candidate / "src" / "core.spx"
    before = base64.b64decode(value["before_b64"], validate=True)
    after = base64.b64decode(value["after_b64"], validate=True)
    if target.read_bytes() != before:
        denied("candidate changed before required drift injection")
    target.write_bytes(after)
    value["applied"] = True
    Path(drift_path).write_text(json.dumps(value, sort_keys=True))
    record(["pilot-drift", command], json.dumps({"after_sha256": value["after_sha256"],
        "before_sha256": value["before_sha256"], "trigger": command}, sort_keys=True).encode(), b"", 0)
args = sys.argv[1:]
if not args: denied("missing semaprax command")
allowed = graph if cfg["lane"] == "semaprax-graph-operational" else source
if args == ["--help"]:
    reserve_log()
    helper = ("pilot-write <.pilot/name.json|.spatch|.wspatch> <base64> writes a bounded patch artifact.\n"
              if cfg["lane"] == "semaprax-graph-operational" else
              "pilot-read <relative .spx> reads source with its SHA-256.\n"
              "pilot-write-source <relative .spx> <expected lowercase sha256> <base64> replaces source conditionally.\n")
    output = ("Allowed commands: " + ", ".join(sorted(allowed)) + "\n" + helper
              + "Use <command> --help for compiler command syntax.\n").encode()
    record(args, output, b"", 0)
    sys.stdout.buffer.write(output)
    raise SystemExit(0)
if args[0] not in allowed: denied("command denied by pilot lane")
for value in args[1:]:
    path = Path(value)
    if path.is_absolute():
        try: path.resolve().relative_to(candidate)
        except ValueError: denied("path escapes candidate")
    elif ".." in path.parts: denied("path escapes candidate")
    if "=" in value:
        _, operand = value.split("=", 1)
        embedded = Path(operand)
        if operand.startswith("/"):
            try: embedded.resolve().relative_to(candidate)
            except ValueError: denied("option path escapes candidate")
        elif ".." in embedded.parts:
            denied("option path escapes candidate")
reserve_log()
if args[0] == "pilot-write":
    if len(args) != 3: denied("pilot-write expects relative path and base64 bytes")
    target = Path(args[1])
    if target.is_absolute() or target.parts[:1] != (".pilot",) or ".." in target.parts:
        denied("pilot-write target denied; use .pilot/name.json, .spatch or .wspatch")
    if target.suffix not in {".json", ".spatch", ".wspatch"}: denied("pilot-write artifact suffix denied")
    try: body = base64.b64decode(args[2], validate=True)
    except Exception: denied("pilot-write body is not canonical base64")
    if len(body) > 1048576: denied("pilot-write body exceeds cap")
    destination = (candidate / target).resolve()
    try: destination.relative_to(candidate)
    except ValueError: denied("pilot-write target escapes candidate")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(body)
    output = b"pilot artifact written\n"
    record(args, output, b"", 0)
    sys.stdout.buffer.write(output)
    raise SystemExit(0)
if args[0] == "pilot-read":
    if len(args) != 2: denied("pilot-read expects one candidate-relative path")
    target = (candidate / Path(args[1])).resolve()
    try: target.relative_to(candidate)
    except ValueError: denied("pilot-read target escapes candidate")
    if (target.is_symlink() or not target.is_file() or target.stat().st_nlink != 1
            or target.stat().st_size > 1048576
            or (target.suffix != ".spx" and target.name != "semaprax.toml")):
        denied("pilot-read target is not a bounded regular file")
    source_bytes = target.read_bytes()
    try: source_text = source_bytes.decode("utf-8")
    except UnicodeDecodeError: denied("pilot-read source is not UTF-8")
    output = json.dumps({"sha256": hashlib.sha256(source_bytes).hexdigest(),
        "text": source_text}, sort_keys=True, ensure_ascii=False).encode()
    # The internal gateway archive retains the exact source bytes read before
    # the drift, while the MCP response includes the matching conditional-edit
    # digest for the source-first agent.
    record(args, source_bytes, b"", 0)
    apply_drift(args)
    sys.stdout.buffer.write(output)
    raise SystemExit(0)
if args[0] == "pilot-write-source":
    if len(args) != 4: denied("pilot-write-source expects relative path, expected sha256 and base64 bytes")
    target = (candidate / Path(args[1])).resolve()
    try: target.relative_to(candidate)
    except ValueError: denied("pilot-write-source target escapes candidate")
    if target.is_symlink() or not target.is_file() or target.stat().st_nlink != 1 or target.suffix != ".spx":
        denied("pilot-write-source target is not a source file")
    expected = args[2]
    if len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected):
        denied("pilot-write-source expected sha256 is invalid")
    if hashlib.sha256(target.read_bytes()).hexdigest() != expected:
        denied("pilot-write-source precondition is stale")
    try: body = base64.b64decode(args[3], validate=True)
    except Exception: denied("pilot-write-source body is not canonical base64")
    if len(body) > 1048576: denied("pilot-write-source body exceeds cap")
    target.write_bytes(body)
    output = b"source written\n"
    record(args, output, b"", 0)
    sys.stdout.buffer.write(output)
    raise SystemExit(0)
def invoke_real(argv):
    try:
        process = subprocess.Popen([cfg["real"], *argv], cwd=candidate, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    except OSError as error:
        return b"", ("compiler gateway spawn failed: " + str(error) + "\n").encode(), 125
    streams = {process.stdout: bytearray(), process.stderr: bytearray()}
    selector = selectors.DefaultSelector()
    for stream in streams: selector.register(stream, selectors.EVENT_READ)
    deadline = time.monotonic() + 60
    failure = None
    try:
        while selector.get_map():
            left = deadline - time.monotonic()
            if left <= 0: failure = "compiler gateway timeout"; break
            for key, _ in selector.select(min(left, 0.25)):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk: selector.unregister(key.fileobj); continue
                streams[key.fileobj].extend(chunk)
                if sum(map(len, streams.values())) > CAP:
                    failure = "compiler gateway output exceeds cap"; break
            if failure: break
        if failure:
            return bytes(streams[process.stdout]), bytes(streams[process.stderr]) + (failure + "\n").encode(), 124
        return bytes(streams[process.stdout]), bytes(streams[process.stderr]), process.wait(timeout=max(0.001, deadline - time.monotonic()))
    finally:
        try: os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError: pass
        process.wait()
        selector.close()
        for stream in streams: stream.close()
output, error, code = invoke_real(args)
record(args, output, error, code)
apply_drift(args)
sys.stdout.buffer.write(output)
sys.stderr.buffer.write(error)
raise SystemExit(code)
'''
    compiler.write_text(wrapper, encoding="utf-8")
    compiler.chmod(0o700)
    return compiler, real, log, config


def graph_mcp_config(state, gateway, gateway_config):
    """Copy the stdlib-only MCP process into private state for graph trials."""
    gateway = Path(gateway).resolve(strict=True)
    if not gateway.is_file() or not os.access(gateway, os.X_OK):
        raise PilotFailure("MCP gateway must be an executable file")
    source = ROOT / "scripts/opencode_agent_task_pilot/mcp_gateway.py"
    server = Path(state) / "mcp_gateway.py"
    body = source.read_bytes()
    server.write_bytes(body)
    server.chmod(0o700)
    if server.read_bytes() != body:
        raise PilotFailure("MCP gateway copy changed")
    wire = Path(state) / "mcp-wire.jsonl"
    return {
        "semaprax": {
            "type": "local",
            "command": ["/usr/bin/python3", str(server), str(gateway), str(wire)],
            "enabled": True,
            "environment": {"SEMAPRAX_PILOT_GATEWAY": gateway_config},
        }
    }, wire


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


def run_tuple(task, lane, trial, opencode, semaprax, evidence, timeout=600, protocol_path=None, model_id=None):
    if lane not in ("semaprax-source-first", "semaprax-graph-operational"):
        raise PilotFailure("pilot lane is not available")
    if trial < 1 or timeout <= 0:
        raise PilotFailure("trial and timeout must be positive")
    evidence = Path(evidence)
    if not evidence.parent.is_dir() or evidence.exists():
        raise PilotFailure(
            "evidence destination must be a new child of an existing directory"
        )
    evidence = evidence.parent.resolve(strict=True) / evidence.name
    if protocol_path is None:
        raise PilotFailure("new pilot execution requires a canonical two-model protocol")
    protocol, protocol_digest = load_frozen_protocol(protocol_path)
    model_identity = next((item for item in protocol["models"] if item["id"] == model_id), None)
    if model_identity is None:
        raise PilotFailure("execution protocol does not contain the selected model id")
    configured_model = model_identity["configured_model"]
    manifest_path = ROOT / "benchmarks/agent-task-comparison-v1/manifest.json"
    if sha(_read_regular(manifest_path, 256 * 1024)) != FROZEN_MANIFEST_SHA256:
        raise PilotFailure("frozen pilot manifest changed")
    _, _, tasks = runner.atc.load_manifest(
        "benchmarks/agent-task-comparison-v1/manifest.json"
    )
    binding = next((x for x in tasks if x["id"] == task), None)
    if binding is None:
        raise PilotFailure(f"unknown task: {task}")
    manifest_digest, task_digest, fixture_digest, frozen_inventory, task_bytes = frozen_input_binding(binding)
    prompt = json.loads(task_bytes.decode("utf-8"))["prompt"]
    original = original_repository_root()
    evidence.mkdir()
    initialize_intervention_ledger(evidence)
    sandbox = None
    candidate = None
    before = None
    baseline_sources = None
    original_before = None
    after = None
    out = b""
    err = b""
    exported = b""
    gateway_log = b""
    mcp_wire = b""
    profile_bytes = b""
    session = None
    compiler_digest = None
    elapsed = None
    failure = None
    candidate_archive = None
    candidate_archive_failure = None
    acceptance_rows = None
    acceptance_error = None
    validation_wall_ns = None
    review_package = None
    presentation = None
    presentation_error = None
    drift_applications = 0
    gateway_diagnostic = {"status": "unavailable", "reason": "gateway was not reached"}
    mcp_metrics = {"status": "unavailable", "reason": "MCP tools/call wire was not available"}
    model_counters = {"status": "unavailable", "reason": "session export was not available"}
    result = None
    try:
        sandbox, candidate = runner.create_sandbox(binding)
        before = snapshot(candidate)
        baseline_sources = collect_source_bytes(candidate)
        if {name: sha(body) for name, body in sorted(baseline_sources.items())} != {
            name: sha(body) for name, body in sorted(frozen_inventory.items())
        }:
            raise PilotFailure("copied fixture inventory differs before dispatch")
        original_before = snapshot(runner.fixture_root_for(binding))
        with tempfile.TemporaryDirectory(prefix="spx-opencode-pilot-") as temp:
            host = Path(temp).resolve()
            config = host / "opencode.json"
            user_home = Path(pwd.getpwuid(os.getuid()).pw_dir).resolve(strict=True)
            profile = host / "seatbelt.sb"
            profile.write_text(
                seatbelt_profile(
                    candidate, (ROOT, original, evidence.parent, user_home), host
                )
            )
            profile_bytes = profile.read_bytes()
            protected_files = {
                ROOT / "benchmarks/agent-task-comparison-v1/manifest.json",
                original / "benchmarks/agent-task-comparison-v1/manifest.json",
            }
            seatbelt_probe(
                profile, candidate / "semaprax.toml", protected_files, candidate, host
            )
            compiler, compiler_digest = provision_semaprax(semaprax, host)
            drift = prepare_drift(binding, candidate, host)
            compiler, real_compiler, gateway, gateway_config = install_gateway(
                compiler, host, candidate, lane, drift
            )
            mcp, wire = graph_mcp_config(host, compiler, gateway_config)
            config.write_text(json.dumps(policy(lane, mcp, configured_model), sort_keys=True))
            env = private_environment(host, config, compiler.parent)
            env["SEMAPRAX_PILOT_GATEWAY"] = gateway_config
            start = time.monotonic_ns()
            try:
                out, err, _ = bounded(
                    sandboxed(
                        opencode,
                        profile,
                        [
                            "run", "--pure", "--agent", AGENT, "--model", configured_model,
                            "--format", "json", "--dir", str(candidate), prompt,
                        ],
                    ),
                    host,
                    timeout,
                    env,
                )
            except PilotFailure as error:
                out, err = error.stdout, error.stderr
                gateway_log = gateway.read_bytes() if gateway.exists() else b""
                mcp_wire = wire.read_bytes() if wire.exists() else b""
                raise
            finally:
                elapsed = time.monotonic_ns() - start
                if drift is not None:
                    drift_applications = int(
                        json.loads(drift.read_text(encoding="utf-8"))["applied"]
                    )
            gateway_log = gateway.read_bytes() if gateway.exists() else b""
            mcp_wire = wire.read_bytes() if wire.exists() else b""
            try:
                mcp_metrics = mcp_tool_metrics(mcp_wire)
            except ValueError as error:
                mcp_metrics = {"status": "unavailable", "reason": str(error)}
            try:
                gateway_diagnostic = gateway_diagnostics(gateway_log)
            except ValueError as error:
                gateway_diagnostic = {"status": "unavailable", "reason": str(error)}
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
            try:
                exported, _, _ = bounded(
                    sandboxed(opencode, profile, ["export", session, "--pure"]),
                    host, timeout, env,
                )
            except PilotFailure as error:
                if not err:
                    err = error.stderr
                gateway_log = gateway.read_bytes() if gateway.exists() else b""
                mcp_wire = wire.read_bytes() if wire.exists() else b""
                raise
            model_counters = provider_usage(exported, configured_model)
            after = snapshot(candidate)
            gateway_log = gateway.read_bytes() if gateway.exists() else b""
            mcp_wire = wire.read_bytes() if wire.exists() else b""
            original_after = snapshot(runner.fixture_root_for(binding))
            validation_started = time.monotonic_ns()
            try:
                acceptance = runner.independent_acceptance(
                    task, candidate, drift_applications, binding, before, after,
                    real_compiler, original_before, original_after, after,
                )
                acceptance_rows = acceptance[0] if isinstance(acceptance, tuple) else acceptance
                if isinstance(acceptance, tuple):
                    review_package = acceptance[1]
            except Exception as error:
                acceptance_error = str(error)
                raise
            finally:
                validation_wall_ns = time.monotonic_ns() - validation_started
    except Exception as error:
        failure = str(error)
        raise
    finally:
        if candidate is not None and after is None:
            try:
                after = snapshot(candidate)
            except Exception:
                after = None
        if candidate is not None and baseline_sources is not None:
            try:
                candidate_archive = archive_candidate(candidate, evidence, baseline_sources)
            except Exception as error:
                candidate_archive_failure = str(error)
        for name, body in (
            ("stdout.jsonl", out), ("stderr.txt", err), ("session.json", exported),
            ("gateway.jsonl", gateway_log), ("mcp-wire.jsonl", mcp_wire),
            ("seatbelt.sb", profile_bytes),
        ):
            exclusive_write(evidence / name, body)
        try:
            presentation = write_presentation_evidence(evidence, prompt, mcp_wire)
        except ValueError as error:
            # Retain the failed tuple and make eligibility fail closed below.
            presentation_error = str(error)
        if gateway_diagnostic["status"] == "unavailable" and gateway_log:
            try:
                gateway_diagnostic = gateway_diagnostics(gateway_log)
            except ValueError as error:
                gateway_diagnostic = {"status": "unavailable", "reason": str(error)}
        if mcp_metrics["status"] == "unavailable" and mcp_wire:
            try:
                mcp_metrics = mcp_tool_metrics(mcp_wire)
            except ValueError as error:
                mcp_metrics = {"status": "unavailable", "reason": str(error)}
        if review_package is not None:
            exclusive_write(evidence / "review-package.json", (json.dumps(review_package, sort_keys=True) + "\n").encode())
        eligibility = compute_eligibility(
            prompt=prompt,
            gateway_log_bytes=gateway_log,
            drift_declared=binding["drift_patch"] is not None,
            evidence_dir=evidence,
        )
        record = {
            "schema": "semaprax.opencode-agent-task-pilot.v1",
            "status": "eligible" if eligibility["eligible"] else "ineligible",
            "reason": None if eligibility["eligible"] else "; ".join(eligibility["reasons"]),
            "eligibility": eligibility,
            "task": task, "lane": lane, "trial": trial, "model": configured_model,
            "protocol_sha256": protocol_digest, "model_identity": model_identity,
            "manifest_sha256": manifest_digest, "task_sha256": task_digest,
            "fixture_inventory_sha256": fixture_digest,
            "semaprax_sha256": compiler_digest, "session_id": session,
            "wall_ns": elapsed, "before": before, "after": after,
            "stdout_sha256": sha(out), "stderr_sha256": sha(err),
            "session_sha256": sha(exported), "gateway_sha256": sha(gateway_log),
            "mcp_wire_sha256": sha(mcp_wire),
            "presentation_sha256": None if presentation is None else presentation["presentation_sha256"],
            "presentation_error": presentation_error,
            "drift_applications": drift_applications,
            "acceptance": acceptance_rows,
            "acceptance_error": acceptance_error,
            "validation_wall_ns": validation_wall_ns,
            "validation_wall_ms": None if validation_wall_ns is None else validation_wall_ns // 1_000_000,
            "review_package": "review-package.json" if review_package is not None else None,
            "candidate_archive": candidate_archive,
            "candidate_archive_error": candidate_archive_failure,
            "provider_usage": model_counters,
            "gateway_diagnostics": gateway_diagnostic,
            "mcp_tool_metrics": mcp_metrics,
            "outcome": "failed" if failure else "completed",
            "failure": failure,
        }
        exclusive_write(evidence / "record.json", (json.dumps(record, sort_keys=True) + "\n").encode())
        if sandbox is not None:
            shutil.rmtree(sandbox, ignore_errors=True)
        result = record
    return result


def main():
    a = argparse.ArgumentParser()
    sub = a.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run", help="run one isolated tuple and archive its evidence")
    run.add_argument("--task", required=True)
    run.add_argument("--lane", required=True)
    run.add_argument("--trial", type=int, required=True)
    run.add_argument("--opencode", default="/opt/homebrew/bin/opencode")
    run.add_argument(
        "--semaprax",
        required=True,
        help="absolute compiler executable to copy into private host state",
    )
    run.add_argument("--evidence-dir", required=True)
    run.add_argument("--timeout", type=int, default=600)
    run.add_argument("--protocol", required=True, help="canonical two-model execution protocol JSON")
    run.add_argument("--model-id", required=True, help="one model identity from --protocol")

    intervene = sub.add_parser(
        "intervene",
        help="append one entry to an already-created trial's operator intervention ledger",
    )
    intervene.add_argument("--evidence-dir", required=True)
    intervene.add_argument("--kind", required=True, choices=sorted(INTERVENTION_KINDS))
    intervene.add_argument("--target", required=True)
    intervene.add_argument("--note", default=None)

    review = sub.add_parser(
        "record-review",
        help="record one blinded reviewer's active review interval for a trial's candidate diff",
    )
    review.add_argument("--evidence-dir", required=True)
    review.add_argument("--reviewer-id", required=True)
    review.add_argument("--started-monotonic-ns", type=int, required=True)
    review.add_argument("--stopped-monotonic-ns", type=int, required=True)
    review.add_argument("--active-ms", type=int, required=True)
    review.add_argument(
        "--blinded",
        action="store_true",
        required=True,
        help="required operator attestation that direct lane/model/runner labels were withheld",
    )
    review.add_argument("--packet", help="blinded packet to bind this submission to the exact candidate")
    review.add_argument("--candidate-digest", help="digest printed in the blinded packet")
    review.add_argument("--verdict", choices=("accept", "reject"), help="reviewer's acceptance verdict")

    packet = sub.add_parser("prepare-review", help="prepare a blinded packet from archived candidate evidence")
    packet.add_argument("--evidence-dir", required=True)
    packet.add_argument("--output", required=True)

    start_review = sub.add_parser(
        "start-review",
        help="freeze a blinded packet and begin an automatically timed reviewer session",
    )
    start_review.add_argument("--evidence-dir", required=True)

    finish_review = sub.add_parser(
        "finish-review",
        help="finish a host-clock timed blinded review session and record its verdict",
    )
    finish_review.add_argument("--evidence-dir", required=True)
    finish_review.add_argument("--reviewer-id", required=True)
    finish_review.add_argument("--verdict", choices=("accept", "reject"), required=True)
    finish_review.add_argument(
        "--blinded",
        action="store_true",
        required=True,
        help="required reviewer attestation that direct lane/model/runner labels were withheld",
    )

    audit = sub.add_parser("audit-cohort", help="audit exact tuple accounting without running a model")
    audit.add_argument("--evidence-root", required=True)
    audit.add_argument("--manifest", default=str(ROOT / "benchmarks/agent-task-comparison-v1/manifest.json"))
    audit.add_argument("--protocol", help="canonical two-model protocol; omit only for historical ineligible evidence")

    n = a.parse_args()
    if n.command == "run":
        output = run_tuple(
            n.task, n.lane, n.trial, n.opencode, n.semaprax, Path(n.evidence_dir), n.timeout,
            n.protocol, n.model_id,
        )
    elif n.command == "intervene":
        output = append_intervention(Path(n.evidence_dir), n.kind, n.target, n.note)
    elif n.command == "record-review":
        output = record_blinded_review(
            Path(n.evidence_dir), n.reviewer_id, n.started_monotonic_ns,
            n.stopped_monotonic_ns, n.active_ms, n.blinded, n.packet, n.candidate_digest,
            n.verdict,
        )
    elif n.command == "prepare-review":
        output = prepare_review_packet(Path(n.evidence_dir), Path(n.output))
    elif n.command == "start-review":
        output = start_blinded_review(Path(n.evidence_dir))
    elif n.command == "finish-review":
        output = finish_blinded_review(Path(n.evidence_dir), n.reviewer_id, n.verdict, n.blinded)
    else:
        output = audit_cohort(Path(n.evidence_root), Path(n.manifest), n.protocol)
        if not output["complete"]:
            raise SystemExit(1)
    print(json.dumps(output, indent=2))


if __name__ == "__main__":
    main()
