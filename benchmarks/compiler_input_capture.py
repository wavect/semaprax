"""Retain runner-selected pre-codec inputs without granting compiler authority.

The agent-side proxy executes the real compiler in its original sandbox. A
runner-side mailbox consumer alone writes evidence outside that sandbox. These
receipts are input retention, not a complete input closure or execution proof.
"""
from __future__ import annotations

import contextlib
import hashlib
import json
import os
import shlex
import shutil
import signal
import stat
import subprocess
import sys
import threading
import time
import uuid
from pathlib import Path
from typing import Any

# The standalone copy resolves only its retained sibling, never candidate code.
if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
import compiler_output_provenance as provenance

SCHEMA = "semaprax.selected-compiler-input-retention.v1"
MAX_MESSAGE_BYTES = 16 * 1024
MAX_RECEIPT_BYTES = 32 * 1024
MAX_REQUESTS = 128
MAX_RETAINED_BYTES = 64 * 1024 * 1024
MAX_SELECTION_BYTES = 8 * 1024 * 1024
WAIT_SECONDS = 30
PROFILES = {"identifier-views.v1", "request-views.v1", "stream-request-views.v1",
            "owned-request.v1", "stream-owned-request.v1", "utf8-owned-request.v1",
            "stream-utf8-owned-request.v1", "bounded-collection-response.v1",
            "bounded-nested-request.v1", "bounded-stream-nested-request.v1"}


def _json(path: Path, document: dict[str, Any], limit: int = MAX_MESSAGE_BYTES) -> None:
    directory_fd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        _json_at(directory_fd, path.name, document, limit)
    finally:
        os.close(directory_fd)


def _json_at(directory_fd: int, name: str, document: dict[str, Any], limit: int = MAX_MESSAGE_BYTES) -> None:
    data = (json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n").encode()
    if len(data) > limit:
        raise ValueError("capture message exceeds byte budget")
    temporary = "." + uuid.uuid4().hex + ".capture-pending"
    try:
        fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                     0o600, dir_fd=directory_fd)
        with os.fdopen(fd, "wb") as output:
            output.write(data)
        # The final name becomes visible only after every byte is written and
        # cannot overwrite earlier evidence, even under a publication race.
        os.link(temporary, name, src_dir_fd=directory_fd, dst_dir_fd=directory_fd, follow_symlinks=False)
    finally:
        try:
            os.unlink(temporary, dir_fd=directory_fd)
        except FileNotFoundError:
            pass


def _options(argv: list[str]) -> dict[str, str] | None:
    """Mirror the closed CLI grammar only; malformed calls reach ordinary CLI."""
    if not argv or argv[0] != "json-codec" or len(argv) not in (8, 10, 12, 14):
        return None
    if not argv[1] or argv[1].startswith("-"):
        return None
    values = {"project": argv[1]}
    for key, value in zip(argv[2::2], argv[3::2]):
        if key not in ("--source", "--type", "--output", "--profile", "--max-string-bytes",
                       "--max-array-items") or key in values:
            return None
        if not value or value.startswith("-"):
            return None
        values[key] = value
    if not all(key in values for key in ("--source", "--type", "--output")):
        return None
    profile = values.get("--profile")
    bound = values.get("--max-string-bytes")
    items = values.get("--max-array-items")
    if profile not in PROFILES and profile is not None:
        return None
    if profile in ("utf8-owned-request.v1", "stream-utf8-owned-request.v1",
                   "bounded-collection-response.v1", "bounded-nested-request.v1",
                   "bounded-stream-nested-request.v1"):
        if bound is None or len(bound) > 2 or not bound.isascii() or not bound.isdigit():
            return None
        if str(int(bound)) != bound or not 1 <= int(bound) <= 64:
            return None
    elif bound is not None:
        return None
    if profile in ("bounded-nested-request.v1", "bounded-stream-nested-request.v1"):
        if items is None or len(items) > 3 or not items.isascii() or not items.isdigit():
            return None
        if str(int(items)) != items or not 1 <= int(items) <= 256:
            return None
    elif items is not None:
        return None
    return values


def _under(candidate: Path, path: Path) -> str:
    # Lexical containment followed by capture_inputs' no-follow handles. Never
    # resolve an untrusted symlink to a different input/output authority.
    return provenance._relative(Path(os.path.abspath(path)).relative_to(candidate).as_posix(), "selected path")


def _selection(candidate: Path, cwd: str, values: dict[str, str], candidate_fd: int) -> list[str]:
    working = Path(os.path.abspath(cwd))
    working.relative_to(candidate)
    project = Path(os.path.abspath(working / values["project"]))
    relative = project.relative_to(candidate)
    # This is the CLI's positional directory spelling; only a real directory
    # can select its manifest. Symlink paths are refused by the snapshotter.
    directory_fd = os.dup(candidate_fd)
    try:
        for index, component in enumerate(relative.parts):
            flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
            if index + 1 < len(relative.parts):
                flags |= os.O_DIRECTORY
            next_fd = os.open(component, flags, dir_fd=directory_fd)
            os.close(directory_fd)
            directory_fd = next_fd
        is_directory = stat.S_ISDIR(os.fstat(directory_fd).st_mode)
    finally:
        os.close(directory_fd)
    if is_directory:
        project = project / "semaprax.toml"
    manifest = _under(candidate, project)
    source = provenance._relative(values["--source"], "codec source")
    return [manifest, _under(candidate, candidate / manifest / ".." / source)]


def _read_message(root: Path, name: str, limit: int = MAX_MESSAGE_BYTES) -> dict[str, Any]:
    fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        return _read_message_at(fd, name, limit)
    finally:
        os.close(fd)


def _read_message_at(directory_fd: int, name: str, limit: int = MAX_MESSAGE_BYTES) -> dict[str, Any]:
    data = provenance._snapshot_read(directory_fd, name, limit)
    if len(data) > limit:
        raise ValueError("capture message exceeds byte budget")
    try:
        document = json.loads(data)
    except RecursionError as error:
        raise ValueError("capture message nesting exceeds decoder bound") from error
    if not isinstance(document, dict):
        raise ValueError("capture message must be an object")
    return document


class Broker:
    """Only bounded no-follow candidate reads and exclusive evidence writes."""
    def __init__(self, candidate: Path, mailbox: Path, evidence: Path, compiler: dict[str, str],
                 workspace: Path | None = None, retained_receipts_enabled: bool = False):
        self.candidate = Path(os.path.abspath(candidate))
        self.workspace = Path(os.path.abspath(workspace if workspace is not None else candidate.parent))
        self.candidate_relative = self.candidate.relative_to(self.workspace)
        self.mailbox, self.evidence, self.compiler = mailbox, evidence, compiler
        self.retained_receipts_enabled = retained_receipts_enabled
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self._watch, daemon=True)
        self.request_receipts: list[dict[str, str]] = []
        self.seen: set[str] = set()
        self.starts: dict[str, dict[str, Any]] = {}
        self.errors: list[str] = []
        self.retained_bytes = 0
        self.capture_wait_ns_proxy_reported = 0
        self.workspace_fd = os.open(self.workspace, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            self.mailbox_fd = os.open(mailbox, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        except BaseException:
            os.close(self.workspace_fd)
            raise

    def _candidate_fd(self) -> int:
        directory_fd = os.dup(self.workspace_fd)
        try:
            for component in self.candidate_relative.parts:
                next_fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                  dir_fd=directory_fd)
                os.close(directory_fd)
                directory_fd = next_fd
            return directory_fd
        except BaseException:
            os.close(directory_fd)
            raise

    def _capture(self, paths: list[str], destination: Path) -> dict[str, Any]:
        limit = min(MAX_SELECTION_BYTES, MAX_RETAINED_BYTES - self.retained_bytes)
        directory_fd = self._candidate_fd()
        try:
            receipt, digest = provenance.capture_inputs(self.candidate, destination, paths, limit,
                authorized_directory_fd=directory_fd)
        finally:
            os.close(directory_fd)
        rows = provenance.validate_input_snapshot(receipt, digest)
        self.retained_bytes += sum(row["bytes"] for row in rows)
        return {"receipt": str(receipt), "sha256": digest, "files": rows}

    def handle(self, identifier: str, request: dict[str, Any]) -> dict[str, Any]:
        if (len(identifier) != 32 or any(c not in "0123456789abcdef" for c in identifier)
                or identifier in self.seen or len(self.seen) >= MAX_REQUESTS):
            raise ValueError("capture request is invalid, replayed, or exceeds count budget")
        self.seen.add(identifier)
        response: dict[str, Any] = {"schema": SCHEMA, "request": identifier,
            "compiler": self.compiler, "input_closure_complete": False,
            "exact_compiler_input_binding": False, "independent_execution_proof": None,
            "repeat_output_sha256": None, "generated_source_classification": None,
            "authored_token_subtraction": None, "fixed_context_tokens": None,
            "actual_billed_usd": None, "selection_scope": "selected manifest and --source only",
            "status": "missing_retention"}
        directory = self.evidence / identifier
        directory.mkdir()
        try:
            operation = request.get("operation")
            if operation == "start":
                argv, cwd = request.get("argv"), request.get("cwd")
                if not isinstance(argv, list) or not all(isinstance(arg, str) for arg in argv) or not isinstance(cwd, str):
                    raise ValueError("capture invocation shape is invalid")
                values = _options(argv)
                receipt_mode = False
                if values is None and self.retained_receipts_enabled:
                    import compiler_retained_receipt as retained
                    values = retained.options(argv)
                    receipt_mode = values is not None
                if values is None:
                    raise ValueError("capture invocation is not supported codec grammar")
                response.update({"argv": argv, "cwd": cwd})
                candidate_fd = self._candidate_fd()
                try:
                    selected = _selection(self.candidate, cwd, values, candidate_fd)
                finally:
                    os.close(candidate_fd)
                response["selected_inputs"] = self._capture(selected, directory / "before")
                response["status"] = "selected_inputs_retained"
                self.starts[identifier] = {"argv": argv, "cwd": cwd, "values": values,
                    "receipt_mode": receipt_mode, "selected_inputs": response["selected_inputs"]}
            elif operation == "finish":
                start = request.get("start")
                if not isinstance(start, str) or start not in self.starts:
                    raise ValueError("capture completion has no retained start")
                invocation = self.starts.pop(start)
                code = request.get("exit_code")
                if isinstance(code, bool) or not isinstance(code, int) or not -128 <= code <= 255:
                    raise ValueError("capture completion exit code must be a process status")
                response.update({"start": start, "exit_code_proxy_reported": code,
                    "compiler_status_independently_observed": False, "status": "completion_reported"})
                if invocation["receipt_mode"]:
                    response["compiler_retained_receipt_requested"] = True
                if code == 0:
                    output = Path(invocation["cwd"]) / invocation["values"]["--output"]
                    response["declared_output_bytes"] = self._capture([_under(self.candidate, output)], directory / "after")
                    response["output_execution_authentication"] = None
                    if invocation["receipt_mode"]:
                        response["compiler_receipt_observation"] = request.get("compiler_receipt_observation")
                        response["compiler_retained_receipt_adoption"] = self._adopt_receipt(
                            identifier, invocation, response, directory)
            elif operation == "measurement":
                wait = request.get("capture_wait_ns_proxy_reported")
                error = request.get("capture_error")
                if (isinstance(wait, bool) or not isinstance(wait, int) or not 0 <= wait < 2**63
                        or (error is not None and (not isinstance(error, str) or len(error) > 1024))):
                    raise ValueError("proxy measurement is not bounded metadata")
                response.update({"status": "proxy_measurement_reported",
                    "capture_wait_ns_proxy_reported": wait,
                    "proxy_measurement_independently_observed": False})
                code = request.get("exit_code_proxy_reported")
                if code is not None:
                    if isinstance(code, bool) or not isinstance(code, int) or not -128 <= code <= 255:
                        raise ValueError("proxy measurement exit code is invalid")
                    response["exit_code_proxy_reported"] = code
                self.capture_wait_ns_proxy_reported += wait
                if error is not None:
                    response["capture_error_proxy_reported"] = error
                    self.errors.append(error)
            else:
                raise ValueError("unknown capture operation")
        except (OSError, ValueError, KeyError) as error:
            response["capture_error"] = str(error)[:1024]
            self.errors.append(response["capture_error"])
        _json(directory / "receipt.json", response, MAX_RECEIPT_BYTES)
        self.request_receipts.append({"path": identifier + "/receipt.json",
            "sha256": provenance.digest(directory / "receipt.json")})
        return response

    def _adopt_receipt(self, identifier: str, invocation: dict, response: dict, directory: Path) -> dict:
        import compiler_retained_receipt as retained
        snapshot = response["declared_output_bytes"]
        if len(snapshot["files"]) != 1:
            raise ValueError("compiler receipt must be one retained output artifact")
        file = snapshot["files"][0]
        fd = os.open(self.evidence, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            raw = provenance._snapshot_read(fd, identifier + "/after/inputs/" + file["path"], MAX_SELECTION_BYTES)
        finally:
            os.close(fd)
        if len(raw) != file["bytes"] or hashlib.sha256(raw).hexdigest() != file["sha256"]:
            raise ValueError("retained compiler artifact differs from captured output")
        report = retained.validate(raw, invocation["argv"], invocation["cwd"], self.compiler,
            response["compiler_receipt_observation"], invocation["selected_inputs"]["files"])
        path = directory / "compiler-receipt-adoption.json"
        _json(path, report, MAX_SELECTION_BYTES)
        return {"path": identifier + "/compiler-receipt-adoption.json", "sha256": provenance.digest(path)}

    def _watch(self) -> None:
        while True:
            stopping = self.stop.wait(0.01)
            try:
                # Both enumeration and reads stay on the creation-time inode.
                # Replacing its path with a symlink cannot redirect the broker.
                for count, name in enumerate(os.listdir(self.mailbox_fd)):
                    if count >= 2 * MAX_REQUESTS:
                        raise ValueError("capture mailbox entry budget exhausted")
                    if not name.endswith(".request.json"):
                        continue
                    identifier = name.removesuffix(".request.json")
                    if identifier in self.seen:
                        continue
                    if len(self.seen) >= MAX_REQUESTS:
                        raise ValueError("capture request count budget exhausted")
                    try:
                        request = _read_message_at(self.mailbox_fd, name)
                        self.handle(identifier, request)
                    except (OSError, ValueError, KeyError) as error:
                        self.errors.append(str(error)[:1024])
                        # Bound malformed/replayed mailbox input as well.
                        self.seen.add(identifier)
            except (OSError, ValueError) as error:
                self.errors.append(str(error)[:1024])
                return
            if stopping:
                return

    def finish(self) -> dict[str, Any]:
        self.stop.set()
        try:
            if self.thread.ident is not None:
                self.thread.join()
        finally:
            if self.mailbox_fd is not None:
                os.close(self.mailbox_fd)
                self.mailbox_fd = None
            if self.workspace_fd is not None:
                os.close(self.workspace_fd)
                self.workspace_fd = None
        return {"schema": SCHEMA, "path": str(self.evidence), "compiler": self.compiler,
            "status": "requests_observed" if self.seen else "no_requests_observed",
            "requests_seen": len(self.seen), "retained_bytes": self.retained_bytes,
            "request_receipts": list(self.request_receipts),
            "capture_errors": self.errors[:MAX_REQUESTS], "unfinished_starts": sorted(self.starts),
            "capture_wait_ns_proxy_reported": self.capture_wait_ns_proxy_reported,
            "capture_wait_independently_observed": False,
            "input_closure_complete": False, "complete_capture_coverage": False,
            "independent_execution_proof": None, "authored_token_subtraction": None,
            "measurement_eligible_for_generated_authorship": False}


def retained_file_inventories(summary: dict[str, Any], metadata: dict[str, Any] | None) -> dict[str, Any]:
    """Count complete retained files separately; never subtract mixed output."""
    import live_campaign_common as common  # runner only, never proxy dispatch
    result: dict[str, Any] = {"schema": "semaprax.retained-codec-file-inventories.v1",
        "status": "unmeasured" if metadata is None else "measured_proxy",
        "scope": "per-invocation selected original input and declared full mixed output files",
        "tokenizer": metadata, "inventories": [], "input_closure_complete": False,
        "exact_compiler_input_binding": False, "independent_execution_proof": None,
        "repeat_output_sha256": None, "generated_source_classification": None,
        "authored_token_subtraction": None, "cumulative_authored_tokens": None,
        "ratio_eligible": False, "actual_billed_usd": None, "fixed_context_tokens": None}
    if metadata is not None:
        common.require_authored_source_tokenizer({"authored_source_tokenizer": metadata})
    evidence = Path(summary["path"])
    root_fd = os.open(evidence, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        bindings = summary["request_receipts"]
        if not isinstance(bindings, list) or len(bindings) > MAX_REQUESTS:
            raise ValueError("retained receipt selection exceeds capture budget")
        seen = set()
        starts = set()
        for binding in bindings:
            path = provenance._relative(binding["path"], "retained request receipt")
            request = path.split('/')[0]
            if (path != request + '/receipt.json' or len(request) != 32
                    or any(c not in '0123456789abcdef' for c in request) or path in seen):
                raise ValueError("retained receipt request selection differs")
            seen.add(path)
            raw = provenance._snapshot_read(root_fd, path, MAX_RECEIPT_BYTES)
            if len(raw) > MAX_RECEIPT_BYTES or hashlib.sha256(raw).hexdigest() != binding["sha256"]:
                raise ValueError("retained request receipt differs from runner binding")
            receipt = json.loads(raw)
            request = binding["path"].split('/')[0]
            if receipt.get("request") != request or receipt.get("compiler") != summary["compiler"]:
                raise ValueError("retained request identity differs")
            for key, kind in (("selected_inputs", "original_selected_inputs"),
                              ("declared_output_bytes", "declared_full_mixed_output")):
                if key not in receipt:
                    continue
                if key == "selected_inputs":
                    starts.add(request)
                elif receipt.get("start") not in starts:
                    raise ValueError("declared output has no retained original input request")
                else:
                    starts.remove(receipt["start"])
                if key == "declared_output_bytes" and receipt.get("compiler_retained_receipt_requested") is True:
                    kind = "compiler_receipt_artifact"
                snapshot = receipt[key]
                relative = provenance._relative(Path(snapshot["receipt"]).relative_to(evidence).as_posix(), "retained snapshot")
                expected = request + ("/before/snapshot.json" if key == "selected_inputs" else "/after/snapshot.json")
                if relative != expected:
                    raise ValueError("retained snapshot is outside its named request")
                snapshot_path = evidence / relative
                files = provenance.validate_input_snapshot(snapshot_path, snapshot["sha256"])
                if files != snapshot["files"]:
                    raise ValueError("retained snapshot file selection differs")
                texts = []
                measured = []
                for file in files:
                    source = provenance._snapshot_read(root_fd, str(Path(relative).parent / 'inputs' / file["path"]), MAX_SELECTION_BYTES)
                    if len(source) != file["bytes"] or hashlib.sha256(source).hexdigest() != file["sha256"]:
                        raise ValueError("tokenized retained bytes differ from authenticated snapshot")
                    texts.append({"path": file["path"], "content": source.decode('utf-8')})
                    measured.append(dict(file))
                counts = common.tokenize_texts(texts, metadata) if metadata is not None else [None] * len(files)
                if len(counts) != len(files) or (metadata is not None and any(type(c) is not int or c < 0 for c in counts)):
                    raise ValueError("retained inventory tokenizer returned invalid counts")
                for file, count in zip(measured, counts):
                    file["tokens"] = count
                result["inventories"].append({"request": request, "start_request": receipt.get("start", request),
                    "request_receipt_sha256": binding["sha256"],
                    "kind": kind, "snapshot_receipt": relative, "snapshot_sha256": snapshot["sha256"],
                    "files": measured, "full_file_proxy_tokens": sum(counts) if metadata is not None else None})
                if kind == "compiler_receipt_artifact":
                    import compiler_retained_receipt as retained
                    adoption = receipt.get("compiler_retained_receipt_adoption")
                    if adoption is None:
                        observed = {"status": "adoption_failed", "error": receipt.get("capture_error", "missing compiler receipt binding"),
                            "input_closure_complete": False, "exact_compiler_input_binding": False, "ratio_eligible": False}
                    else:
                        expected = request + "/compiler-receipt-adoption.json"
                        if adoption.get("path") != expected:
                            raise ValueError("compiler adoption report escapes its request")
                        raw_report = provenance._snapshot_read(root_fd, expected, MAX_SELECTION_BYTES)
                        if hashlib.sha256(raw_report).hexdigest() != adoption["sha256"]:
                            raise ValueError("compiler adoption report differs from runner binding")
                        observed = retained.measure(source, json.loads(raw_report), metadata)
                        observed["adoption_report_sha256"] = adoption["sha256"]
                    result.setdefault("compiler_receipts", []).append(observed)
    finally:
        os.close(root_fd)
    if not result["inventories"]:
        result["status"] = "no_retained_snapshots"
    return result


@contextlib.contextmanager
def _captured_compiler(candidate: Path, workspace: Path, artifacts: Path,
                       label: str, settings: dict[str, Any], binary: Path, row: dict[str, Any]):
    candidate, workspace, artifacts = (Path(os.path.abspath(path)) for path in (candidate, workspace, artifacts))
    source = settings.get("compiler_source_commit")
    if not isinstance(source, str) or len(source) != 40 or any(c not in "0123456789abcdef" for c in source):
        raise ValueError("compiler retention requires the exact compiler source commit")
    real = binary.resolve(strict=True)
    wanted = settings["source_binary_sha256"]
    if provenance.digest(real) != wanted:
        raise ValueError("compiler retention binary differs from immutable plan")
    evidence = artifacts / "compiler-input-retention" / label
    if evidence.resolve().is_relative_to(workspace.resolve()):
        raise ValueError("compiler retention evidence must be outside the authoring workspace")
    if workspace.is_symlink() or not workspace.is_dir():
        raise ValueError("compiler retention workspace must be a real directory")
    evidence.parent.mkdir(parents=True, exist_ok=True)
    evidence.mkdir()
    tools = evidence / "tools"
    tools.mkdir()
    tool_hashes = {}
    optional = "compiler_retained_receipt.py"
    snapshot = settings.get("harness_source_snapshot")
    if snapshot is not None and not isinstance(snapshot, dict):
        raise ValueError("compiler capture harness snapshot is malformed")
    inventories = [settings.get("harness_source_files_sha256"),
        snapshot.get("files_sha256") if snapshot is not None else None]
    supplied = [inventory for inventory in inventories if inventory is not None]
    enabled = any(isinstance(inventory, dict) and "benchmarks/" + optional in inventory for inventory in supplied)
    names = ("compiler_input_capture.py", "compiler_output_provenance.py") + ((optional,) if enabled else ())
    for name in names:
        source_path = Path(__file__).with_name(name)
        if source_path.is_symlink() or not source_path.is_file():
            raise ValueError("compiler capture tool must be a regular harness source")
        contents = source_path.read_bytes()
        digest = hashlib.sha256(contents).hexdigest()
        snapshot = settings.get("harness_source_snapshot")
        if snapshot is not None and not isinstance(snapshot, dict):
            raise ValueError("compiler capture harness snapshot is malformed")
        inventories = [settings.get("harness_source_files_sha256"),
                       snapshot.get("files_sha256") if snapshot is not None else None]
        supplied = [inventory for inventory in inventories if inventory is not None]
        if (not supplied or any(not isinstance(inventory, dict)
                or inventory.get("benchmarks/" + name) != digest for inventory in supplied)):
            raise ValueError("compiler capture tool differs from immutable harness plan")
        (tools / name).write_bytes(contents)
        tool_hashes[name] = digest
    mailbox = workspace / (".compiler-input-capture-" + uuid.uuid4().hex)
    mailbox.mkdir()
    compiler = {"source_commit": source, "binary_sha256": wanted, "real_binary": str(real)}
    broker = None
    config = tools / "config.json"
    try:
        broker = Broker(candidate, mailbox, evidence, compiler, workspace, retained_receipts_enabled=enabled)
        _json(config, {"candidate": str(Path(os.path.abspath(candidate))), "mailbox": str(mailbox),
            "evidence": str(evidence.resolve()), "compiler": compiler, "retained_receipts_enabled": enabled})
        proxy = tools / "semaprax"
        proxy.write_text("#!/bin/sh\nexec " + shlex.quote(sys.executable) + " -I "
            + shlex.quote(str(tools / "compiler_input_capture.py")) + " " + shlex.quote(str(config)) + ' "$@"\n')
        proxy.chmod(0o755)
        row["compiler_authoring_proxy"] = {"path": str(proxy), "sha256": provenance.digest(proxy),
            "tools_sha256": tool_hashes, "config_sha256": provenance.digest(config),
            "real_compiler": compiler, "proxy_is_compiler_binary": False,
            "scope": "SEMAPRAX authoring only"}
        broker.thread.start()
    except BaseException:
        try:
            if broker is not None:
                broker.finish()
        finally:
            if mailbox.is_symlink():
                mailbox.unlink()
            elif mailbox.exists():
                shutil.rmtree(mailbox)
        raise
    try:
        yield proxy
    finally:
        row["compiler_input_retention"] = broker.finish()
        try:
            row["retained_compiler_file_inventories"] = retained_file_inventories(
                row["compiler_input_retention"], settings.get("authored_source_tokenizer"))
        except (OSError, ValueError, KeyError, RuntimeError, UnicodeError, subprocess.SubprocessError) as error:
            row["retained_compiler_file_inventories"] = {"status": "measurement_failed",
                "error": str(error)[:1024], "inventories": [], "input_closure_complete": False,
                "authored_token_subtraction": None, "generated_source_classification": None,
                "ratio_eligible": False}
        try:
            _json(evidence / "summary.json", row["compiler_input_retention"], 1024 * 1024)
        except (OSError, ValueError) as error:
            row["compiler_input_retention"]["summary_write_error"] = str(error)[:1024]
        try:
            # Never follow a replaced channel into an unrelated directory.
            if mailbox.is_symlink():
                mailbox.unlink()
            elif mailbox.exists():
                shutil.rmtree(mailbox)
        except OSError as error:
            row["compiler_input_retention"]["mailbox_cleanup_error"] = str(error)[:1024]


@contextlib.contextmanager
def authoring_compiler(arm: str, candidate: Path, workspace: Path, artifacts: Path,
                       label: str, settings: dict[str, Any], binary: Path, row: dict[str, Any]):
    """Swap only SEMAPRAX authoring's compiler path; acceptance keeps the real one.

    Infrastructure admission failures preserve the ordinary paid attempt and
    acceptance. They never establish complete or missing-input-free evidence.
    """
    if arm != "semaprax":
        yield binary
        return
    entered = False
    try:
        with _captured_compiler(candidate, workspace, artifacts, label, settings, binary, row) as proxy:
            entered = True
            yield proxy
    except (OSError, ValueError, KeyError, RuntimeError) as error:
        if entered:
            raise
        row["compiler_input_retention"] = {"schema": SCHEMA, "status": "unavailable",
            "capture_error": str(error)[:1024], "input_closure_complete": False,
            "complete_capture_coverage": False, "independent_execution_proof": None,
            "measurement_eligible_for_generated_authorship": False,
            "authored_token_subtraction": None}
        yield binary


def _publish_request(config: dict[str, Any], document: dict[str, Any]) -> str:
    identifier = uuid.uuid4().hex
    mailbox = Path(config["mailbox"])
    fd = os.open(mailbox, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        _json_at(fd, identifier + ".request.json", document)
    finally:
        os.close(fd)
    return identifier


def _request(config: dict[str, Any], document: dict[str, Any]) -> str:
    # The authoritative acknowledgement is outside the agent workspace, so
    # mailbox writers cannot forge successful retention.
    identifier = _publish_request(config, document)
    receipt = Path(config["evidence"]) / identifier / "receipt.json"
    deadline = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < deadline:
        if receipt.exists():
            response = _read_message(receipt.parent, receipt.name, MAX_RECEIPT_BYTES)
            if response.get("request") != identifier or response.get("compiler") != config["compiler"]:
                raise ValueError("capture acknowledgement binding differs")
            if response.get("capture_error"):
                raise ValueError(response["capture_error"])
            if document["operation"] == "start":
                if response.get("argv") != document["argv"] or response.get("cwd") != document["cwd"]:
                    raise ValueError("capture acknowledgement invocation differs")
                selected = response["selected_inputs"]
                provenance.validate_input_snapshot(Path(selected["receipt"]), selected["sha256"])
            return identifier
        time.sleep(0.01)
    raise ValueError("runner input retention acknowledgement unavailable")


def _measurement(config: dict[str, Any], wait_ns: int, error: str | None, code: int | None = None) -> None:
    try:
        _publish_request(config, {"operation": "measurement",
            "capture_wait_ns_proxy_reported": wait_ns, "capture_error": error,
            "exit_code_proxy_reported": code})
    except (OSError, ValueError, KeyError):
        # Best effort without extra tool context or compiler stderr. Complete
        # capture coverage is unavailable even when no broker error was seen.
        pass


def proxy_main(config: dict[str, Any], argv: list[str]) -> int:
    compiler = config["compiler"]
    binary = Path(compiler["real_binary"])
    binary_before = provenance.digest(binary)
    if binary_before != compiler["binary_sha256"]:
        raise ValueError("actual compiler changed after campaign plan")
    receipt_mode = False
    if config.get("retained_receipts_enabled") is True:
        import compiler_retained_receipt as retained
        receipt_mode = retained.options(argv) is not None
    if _options(argv) is None and not receipt_mode:
        os.execv(str(binary), [str(binary), *argv])
        raise AssertionError("exec returned")
    start = None
    wait_started = time.monotonic_ns()
    capture_error = None
    try:
        start = _request(config, {"operation": "start", "argv": argv, "cwd": os.getcwd()})
    except (OSError, ValueError, KeyError) as error:
        # Capture failure never substitutes a compiler/refusal result. It is a
        # measurement limitation, retained when the broker received a request.
        capture_error = str(error)[:1024]
    _measurement(config, time.monotonic_ns() - wait_started, capture_error)
    # Inherit the original cwd/env/stdin/stdout/stderr and process group. No
    # runner-side compiler execution, shell, or extra ambient capability.
    code = subprocess.run([str(binary), *argv], check=False).returncode
    wait_started = time.monotonic_ns()
    capture_error = None
    if start is not None:
        try:
            finish = {"operation": "finish", "start": start, "exit_code": code}
            if receipt_mode:
                finish["compiler_receipt_observation"] = {"observer": "sandboxed_harness_proxy",
                    "binary_sha256_before": binary_before, "binary_sha256_after": provenance.digest(binary),
                    "invocation_sha256": retained.invocation_sha256(str(binary), argv, os.getcwd()), "exit_code": code}
            _request(config, finish)
        except (OSError, ValueError, KeyError) as error:
            capture_error = str(error)[:1024]
    _measurement(config, time.monotonic_ns() - wait_started, capture_error, code)
    if code < 0:
        if -code not in (signal.SIGKILL, signal.SIGSTOP):
            signal.signal(-code, signal.SIG_DFL)
        os.kill(os.getpid(), -code)
    return code


if __name__ == "__main__":
    try:
        configuration = _read_message(Path(sys.argv[1]).parent, Path(sys.argv[1]).name)
        sys.exit(proxy_main(configuration, sys.argv[2:]))
    except (OSError, ValueError, KeyError) as failure:
        print(f"compiler proxy refused: {failure}", file=sys.stderr)
        sys.exit(2)
