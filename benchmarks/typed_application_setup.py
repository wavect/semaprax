#!/usr/bin/env python3
"""Install the reviewed typed public specimens using actual checked generators.

This operator-only helper never seeds paid candidates, runs a model, fetches a
dependency, or qualifies acceptance. Both generator outputs must replay against
the same unchanged retained bootstrap before a regular completed project is returned.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import signal
import stat
import subprocess
import sys
import time
import tomllib

import compiler_output_provenance as provenance
from typed_application_setup_support import dependencies

MAX_BYTES = 8 * 1024 * 1024
MAX_GENERATED_BYTES = 2 * 1024 * 1024
PROFILE = "language-command-io.collection-record.v1"
EXAMPLES = {
    "shiftsim": {"directory": "shiftsim-typed-record-successor", "prefix": "shiftsim",
                 "request_profile": "stream-owned-request.v1", "request_flags": [],
                 "sources": ["app", "model", "order", "request", "response", "schedule", "tests"]},
    "catalog": {"directory": "catalog-scoped-record-successor", "prefix": "catalog",
                "request_profile": "stream-utf8-owned-request.v1",
                "request_flags": ["--max-string-bytes", "16"],
                "sources": ["app", "model", "order", "publication", "request", "response", "restock", "tests"]},
}


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def directory_fd(path: Path) -> int:
    path = path.absolute()
    fd = os.open(path.anchor, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in path.parts[1:]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            os.close(fd)
            fd = child
        return fd
    except BaseException:
        os.close(fd)
        raise


def create_new_directory(path: Path) -> None:
    parent_fd = directory_fd(path.absolute().parent)
    try:
        os.mkdir(path.name, dir_fd=parent_fd)
    finally:
        os.close(parent_fd)


def regular_fd(path: Path) -> int:
    path = path.absolute()
    # All components, not just the final file, are opened without symlinks.
    parent_fd = os.open(path.anchor, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in path.parts[1:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
            os.close(parent_fd)
            parent_fd = child
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent_fd)
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            os.close(fd)
            raise ValueError("input must be a regular file")
        return fd
    finally:
        os.close(parent_fd)


def read_regular(path: Path, limit: int = MAX_BYTES) -> bytes:
    with os.fdopen(regular_fd(path), "rb") as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f"bounded regular input exceeds {limit} bytes")
    return data


def digest_regular(path: Path) -> str:
    with os.fdopen(regular_fd(path), "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def selected_paths(application: str) -> list[str]:
    example = EXAMPLES[application]
    return sorted(["semaprax.toml", "src/app.command.spx", "fixtures/request.json",
                   *(f"src/{name}.spx" for name in example["sources"])])


def validate_manifest(application: str, data: bytes) -> dict:
    example = EXAMPLES[application]
    manifest = tomllib.loads(data.decode("utf-8"))
    expected = [f"src/{name}.spx" for name in example["sources"]]
    if (manifest.get("schema") != "semaprax.manifest.v1"
            or manifest.get("package", {}).get("profile") != PROFILE
            or manifest.get("modules", {}).get("sources") != expected
            or manifest.get("modules", {}).get("entry") != example["prefix"] + ".app"
            or manifest.get("modules", {}).get("tests") != [example["prefix"] + ".tests"]
            or manifest.get("exports", {}).get("web") != [example["prefix"] + ".command"]
            or manifest.get("command") != {"function": example["prefix"] + ".command",
                                           "input": "argv-utf8+stdin-stream.v1"}
            or manifest.get("capabilities", {}).get("required") != [
                "process.args.read", "process.stderr.write", "process.stdin.read", "process.stdout.write"]
            or manifest.get("targets", {}).get("matrix") != ["native64"]):
        raise ValueError("public specimen manifest differs from its closed v31 route")
    return manifest


def verify_build_receipt(path: Path, source: str, binary_hash: str) -> dict:
    """Retain byte bindings; caller metadata never proves hosted execution."""
    data = read_regular(path, 1024 * 1024)
    receipt = json.loads(data)
    required = {"schema", "compiler_source_commit", "compiler_binary_sha256", "build_command", "build_log"}
    if (not isinstance(receipt, dict) or set(receipt) != required
            or receipt["schema"] != "semaprax.loglens.compiler-build.v1"
            or receipt["compiler_source_commit"] != source
            or receipt["compiler_binary_sha256"] != binary_hash):
        raise ValueError("compiler build receipt requires the exact source/binary subject")
    command, log = receipt["build_command"], receipt["build_log"]
    if (not isinstance(command, list) or not command
            or any(not isinstance(value, str) or not value.strip() or "\0" in value for value in command)
            or not isinstance(log, dict) or set(log) != {"path", "sha256"}):
        raise ValueError("invalid retained build command/log")
    relative = PurePosixPath(log["path"]) if isinstance(log["path"], str) else PurePosixPath("/")
    if (relative.is_absolute() or not relative.parts or ".." in relative.parts
            or not re.fullmatch(r"[0-9a-f]{64}", str(log["sha256"]))):
        raise ValueError("build log requires a bounded relative regular path and SHA-256")
    log_data = read_regular(path.absolute().parent / relative)
    if sha(log_data) != log["sha256"]:
        raise ValueError("retained build log changed")
    return {"receipt_sha256": sha(data), "build_log_sha256": sha(log_data),
            "build_command": command,
            "provenance": "caller-supplied byte bindings; hosted build/gate provenance requires operator verification"}


def generator_jobs(application: str) -> list[tuple[str, str, str, list[str]]]:
    example = EXAMPLES[application]
    return [("src/request.spx", example["prefix"] + ".request", example["request_profile"], example["request_flags"]),
            ("src/response.spx", example["prefix"] + ".report", "bounded-collection-response.v1",
             ["--max-string-bytes", "16"])]


def qualification_scaffolding(application: str) -> dict[str, bytes]:
    """Explicit operator scripts, never application logic or paid-arm seeds."""
    return {
        "build.sh": ("#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n"
                     ": \"${SEMAPRAX_BIN:?exact pinned compiler required}\"\nmkdir -p dist\n"
                     "\"$SEMAPRAX_BIN\" check --manifest-path semaprax.toml\n"
                     f"\"$SEMAPRAX_BIN\" build --manifest-path semaprax.toml --target native --output dist/{application}\n").encode(),
        "test.sh": ("#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n"
                    ": \"${SEMAPRAX_BIN:?exact pinned compiler required}\"\n"
                    "\"$SEMAPRAX_BIN\" test .\n").encode(),
        "run.sh": f"#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nexec dist/{application}\n".encode(),
    }


def derive_pair(application: str, compiler: Path, bootstrap: Path, generated: Path,
                run, unchanged, on_replayed=None) -> dict[str, bytes]:
    """No installation is possible until both independent ordinary replays pass."""
    candidates = {}
    for index, (module, identity, profile, flags) in enumerate(generator_jobs(application)):
        first, replay = generated / f"{index}.spx", generated / f"{index}.replay.spx"
        argv = [str(compiler), "json-codec", str(bootstrap / "semaprax.toml"),
                "--source", module, "--type", identity, "--profile", profile, *flags]
        for label, output in (("derive", first), ("replay", replay)):
            unchanged()
            run([*argv, "--output", str(output)], f"{label}-{index}")
            unchanged()
        original = read_regular(first, MAX_GENERATED_BYTES)
        if not original or original != read_regular(replay, MAX_GENERATED_BYTES):
            raise ValueError("actual generator output differs from same-bootstrap replay")
        candidates[module] = original
        if on_replayed is not None:
            on_replayed(module, original)
    return candidates


def record_completion(project: Path, entry: Path, inventory: list[dict]) -> None:
    # Completion is an exclusive receipt in the operator-owned namespace, not
    # an atomic directory transaction. Every qualification input stays regular.
    if project.parent != entry.parent:
        raise ValueError("completion must select the private sibling project")
    parent_fd = directory_fd(project.absolute().parent)
    try:
        if not stat.S_ISDIR(os.stat(project.name, dir_fd=parent_fd, follow_symlinks=False).st_mode):
            raise ValueError("completion must select a real completed project")
        fd = os.open(entry.name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                     0o600, dir_fd=parent_fd)
        created = os.fstat(fd)
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as stream:
                json.dump({"schema": "semaprax.typed-application-completion.v1",
                           "project": project.name, "source_inventory": inventory,
                           "runtime_qualification": None}, stream, sort_keys=True)
                stream.write("\n")
                stream.flush()
                os.fsync(stream.fileno())
        except BaseException:
            current = os.stat(entry.name, dir_fd=parent_fd, follow_symlinks=False)
            if (current.st_dev, current.st_ino) == (created.st_dev, created.st_ino):
                os.unlink(entry.name, dir_fd=parent_fd)
            raise
    finally:
        os.close(parent_fd)


def install(args: argparse.Namespace) -> Path:
    repo = args.repo.absolute()
    if repo.is_symlink() or not repo.is_dir():
        raise ValueError("source checkout must be a real directory")
    source = args.compiler_source
    if not re.fullmatch(r"[0-9a-f]{40}", source) or not re.fullmatch(r"[0-9a-f]{64}", args.compiler_sha256):
        raise ValueError("exact source commit and lowercase binary SHA-256 are required")
    if not isinstance(args.timeout_seconds, int) or not 1 <= args.timeout_seconds <= 1800:
        raise ValueError("setup command timeout must be1..1800 seconds")
    compiler = args.compiler.absolute()
    build_binding = verify_build_receipt(args.compiler_build_receipt, source, args.compiler_sha256)
    retained = {}

    def git(*arguments):
        return subprocess.check_output(["git", "-C", str(repo), *arguments])

    def subject():
        if git("rev-parse", "HEAD").decode().strip() != source or digest_regular(compiler) != args.compiler_sha256:
            raise ValueError("compiler source/binary changed")
        if verify_build_receipt(args.compiler_build_receipt, source, args.compiler_sha256) != build_binding:
            raise ValueError("compiler build receipt changed")
        for relative, data in retained.items():
            if read_regular(repo / relative) != data:
                raise ValueError("selected committed setup source changed")

    def committed(relative: str) -> bytes:
        tree = git("ls-tree", source, "--", relative).decode().split()
        if not tree or tree[0] not in ("100644", "100755"):
            raise ValueError("selected source is not a committed regular file")
        data = git("show", f"{source}:{relative}")
        if read_regular(repo / relative) != data:
            raise ValueError("selected setup input differs from committed source")
        retained[relative] = data
        return data

    subject()
    for relative in ("benchmarks/typed_application_setup.py", "benchmarks/compiler_output_provenance.py",
                     "benchmarks/typed_application_setup_support/dependencies.py",
                     "benchmarks/typed_application_setup_support/installation.py"):
        committed(relative)
    prefix = "examples/" + EXAMPLES[args.application]["directory"] + "/"
    selected = selected_paths(args.application)
    files = {relative: committed(prefix + relative) for relative in selected}
    manifest = validate_manifest(args.application, files["semaprax.toml"])
    registry = dependencies.parse_registry(committed("src/project/standard_dependencies.rs").decode())
    packages = json.loads(committed("std/packages.json"))["packages"]
    directories = {row["module"]: row["directory"] for row in packages}
    if len(directories) != len(packages) or set(directories) != set(registry):
        raise ValueError("package catalog differs from actual bundled registry")
    selected_dependencies = dependencies.closure(manifest, registry)
    for name in selected_dependencies:
        package = registry[name]
        directory = directories[name]
        if not re.fullmatch(r"[a-z0-9-]+", directory):
            raise ValueError("invalid bundled package directory")
        root = "std/" + directory + "/"
        included = package["source"]
        if not included.startswith(root):
            raise ValueError("bundled include differs from package catalog")
        dependency_manifest = tomllib.loads(committed(root + "semaprax.toml").decode())
        dependencies.validate_package(package, dependency_manifest, included[len(root):])
        committed(included)
    subject()
    output = args.output.absolute()
    if output.resolve().is_relative_to(repo.resolve()) or output.exists() or output.is_symlink():
        raise ValueError("output must be a new external operator-owned evidence directory")
    create_new_directory(output)  # Exclusive, no-follow ancestors, no previous evidence overwritten.
    result = {"schema": "semaprax.typed-application-setup.v1", "status": "failed",
              "application": args.application, "profile": PROFILE,
              "compiler_source_commit": source, "compiler_binary_sha256": args.compiler_sha256,
              "compiler_build_receipt": build_binding, "runtime_qualification": None,
              "input_capture_complete_compiler_closure": False,
              "generated_output_is_wholly_compiler_authored": False,
              "model_authored_tokens": None, "fixed_context_tokens": None, "billed_usd": None,
              "stage": "retain-original-inputs",
              "commands": []}
    try:
        selected_receipt, selected_sha = provenance.capture_inputs(repo, output / "selected-inputs", sorted(retained))
        provenance.validate_input_snapshot(selected_receipt, selected_sha)
        result["selected_input_snapshot"] = {"path": str(selected_receipt), "sha256": selected_sha}
        result["bundled_dependencies"] = selected_dependencies
        result["bundled_declared_dependency_closure_complete"] = True
        bootstrap = output / "bootstrap"
        bootstrap.mkdir()
        for relative, data in files.items():
            destination = bootstrap / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            with destination.open("xb") as stream:
                stream.write(data)
        authored, authored_sha = provenance.capture_inputs(bootstrap, output / "authored-inputs", selected)
        result["authored_input_snapshot"] = {"path": str(authored), "sha256": authored_sha}
        inventory = provenance.validate_input_snapshot(authored, authored_sha)

        def unchanged():
            subject()
            provenance.validate_input_snapshot(selected_receipt, selected_sha)
            provenance.validate_input_snapshot(authored, authored_sha)
            for row in inventory:
                if sha(read_regular(bootstrap / row["path"])) != row["sha256"]:
                    raise ValueError("bootstrap changed before both checked derivations completed")

        def run(argv, label, cwd=bootstrap):
            subject()
            result["stage"] = label
            row = {"argv": argv, "cwd": str(cwd), "started_unix_ns": time.time_ns()}
            result["commands"].append(row)
            try:
                with (output / f"{label}.stdout").open("xb") as stdout, (output / f"{label}.stderr").open("xb") as stderr:
                    child = subprocess.Popen(argv, cwd=cwd, stdout=stdout, stderr=stderr,
                                             env={key: value for key, value in os.environ.items()
                                                  if key not in ("PYTHONPATH", "PYTHONHOME", "SEMAPRAX_BIN")},
                                             start_new_session=True)
                    try:
                        row["exit_code"] = child.wait(timeout=args.timeout_seconds)
                    except BaseException:
                        try:
                            os.killpg(child.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                        child.wait()
                        raise
                if row["exit_code"] != 0:
                    raise ValueError(f"{label} failed; compiler status/output retained")
                subject()
            finally:
                row["finished_unix_ns"] = time.time_ns()
                row["retained_output"] = [{"path": f"{label}.{suffix}",
                                            "sha256": digest_regular(output / f"{label}.{suffix}")}
                                           for suffix in ("stdout", "stderr")
                                           if (output / f"{label}.{suffix}").is_file()]

        run([str(compiler), "check", "--manifest-path", str(bootstrap / "semaprax.toml")], "bootstrap-check")
        generated = output / "generated"
        generated.mkdir()
        result["generated_outputs"] = []

        def replayed(module, data):
            result["generated_outputs"].append({"path": module, "sha256": sha(data), "bytes": len(data)})

        candidates = derive_pair(args.application, compiler, bootstrap, generated, run, unchanged, replayed)
        unchanged()
        result["stage"] = "install-source"
        complete = output / "installed-project"
        complete.mkdir()
        for relative, data in files.items():
            if relative == "src/app.command.spx":
                continue
            data = candidates.get(relative, files["src/app.command.spx"] if relative == "src/app.spx" else data)
            destination = complete / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            with destination.open("xb") as stream:
                stream.write(data)
        scaffold = qualification_scaffolding(args.application)
        for relative, data in scaffold.items():
            with (complete / relative).open("xb") as stream:
                stream.write(data)
            (complete / relative).chmod(0o755)
        result["operator_scaffolding"] = [{"path": relative, "sha256": sha(data), "bytes": len(data)}
                                          for relative, data in sorted(scaffold.items())]
        run([str(compiler), "fmt", str(complete / "semaprax.toml"), "--check"], "canonical-check", complete)
        run([str(compiler), "check", "--manifest-path", str(complete / "semaprax.toml")], "installed-check", complete)
        subject()
        installed_paths = sorted((set(files) - {"src/app.command.spx"}) | set(scaffold))
        # The closed installed-source selection excludes compiler-created cache,
        # lock and build artifacts. It includes every declared source and all
        # three operator scripts; none of those files can disappear silently.
        result["installed_source_inventory"] = [{"path": relative, "sha256": sha(read_regular(complete / relative))}
                                                for relative in installed_paths]
        installed_receipt, installed_sha = provenance.capture_inputs(
            complete, output / "installed-inputs", [row["path"] for row in result["installed_source_inventory"]])
        provenance.validate_input_snapshot(installed_receipt, installed_sha)
        result["installed_input_snapshot"] = {"path": str(installed_receipt), "sha256": installed_sha}
        result["status"] = "checked_source_runtime_qualification_pending"
        result["stage"] = "source-checks-complete"
    except BaseException as error:
        result["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        write_json(output / "setup-result.json", result)
    # Return the regular tree only after source checks and receipt persistence.
    # This directory belongs to the operator; no model writes inside it. Failed
    # stages remain evidence, without a completion receipt or qualification.
    try:
        record_completion(complete, output / "completion-receipt.json", result["installed_source_inventory"])
    except BaseException as error:
        write_json(output / "completion-failure.json", {"error": f"{type(error).__name__}: {error}"})
        raise
    return complete


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--application", required=True, choices=EXAMPLES)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--compiler", required=True, type=Path)
    parser.add_argument("--compiler-source", required=True)
    parser.add_argument("--compiler-sha256", required=True)
    parser.add_argument("--compiler-build-receipt", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--verify-installation", action="store_true",
                        help="read-only byte recheck of an existing completed output; no compiler execution or qualification")
    parser.add_argument("--timeout-seconds", type=int, default=1800)
    args = parser.parse_args()
    if args.verify_installation:
        from typed_application_setup_support import installation
        print(installation.verify(args, api=sys.modules[__name__]))
    else:
        print(install(args))


if __name__ == "__main__":
    main()
