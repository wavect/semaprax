#!/usr/bin/env python3
"""Run two authenticated evaluation images sequentially; this grants no admission."""
import argparse
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import stat
import subprocess
import tempfile


PACKAGE = Path(__file__).resolve().with_suffix("")
SCHEMA = "semaprax.opt904-paired-observation-runner.v1"
MARKER = "OPT904_MATCHED_JSON "


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(65536):
            value.update(chunk)
    return value.hexdigest()


def available_memory():
    if Path("/proc/meminfo").exists():
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) * 1024
    try:
        return os.sysconf("SC_AVPHYS_PAGES") * os.sysconf("SC_PAGE_SIZE")
    except (ValueError, OSError):
        return None


def logged_process(argv, worktree, env, log, lock):
    memory_log = log.with_suffix(".memory.jsonl")
    minimum = None
    samples = 0
    with log.open("xb") as stream, memory_log.open("x") as memory:
        # The build or test image retains the lock if the coordinating parent dies.
        with subprocess.Popen(argv, cwd=worktree, env=env, stdout=stream,
                              stderr=subprocess.STDOUT, pass_fds=(lock,)) as process:
            while True:
                available = available_memory()
                memory.write(json.dumps({
                    "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                    "process_pid": process.pid, "host_available_memory_bytes": available,
                }) + "\n")
                memory.flush()
                samples += 1
                if available is not None:
                    minimum = available if minimum is None else min(minimum, available)
                try:
                    status = process.wait(timeout=5)
                    break
                except subprocess.TimeoutExpired:
                    pass
    return {"status": status, "log_sha256": file_digest(log),
            "host_memory_log": memory_log.name, "host_memory_log_sha256": file_digest(memory_log),
            "host_memory_samples": samples, "minimum_sampled_host_available_memory_bytes": minimum,
            "host_memory_sample_interval_seconds": 5}


def binary_snapshot(binary, worktree, target, host):
    resolved = binary.resolve(strict=True)
    require(not binary.is_symlink() and binary.is_file() and os.access(binary, os.X_OK)
            and target.resolve().is_relative_to(worktree.resolve())
            and resolved.is_relative_to((target / host / "debug" / "deps").resolve()),
            "unexpected library test executable identity")
    before = binary.stat()
    sha256 = file_digest(binary)
    after = binary.stat()
    identity = lambda value: (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns)
    require(before.st_size > 0 and identity(before) == identity(after),
            "test executable changed during authentication")
    return {"path": str(binary.relative_to(worktree)), "bytes": after.st_size,
            "sha256": sha256, "device": after.st_dev, "inode": after.st_ino,
            "mtime_ns": after.st_mtime_ns}


def emitted_library_test(log, worktree, target, host):
    candidates = []
    finished = []
    with log.open(encoding="utf-8") as stream:
        for line in stream:
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if not isinstance(message, dict):
                continue
            if message.get("reason") == "build-finished":
                finished.append(message.get("success"))
            artifact = message.get("target", {})
            if (message.get("reason") == "compiler-artifact" and message.get("executable")
                    and message.get("profile", {}).get("test")
                    and message.get("manifest_path") == str(worktree / "Cargo.toml")
                    and artifact.get("name") == "semaprax" and artifact.get("kind") == ["lib"]
                    and artifact.get("src_path") == str(worktree / "src/lib.rs")):
                candidates.append(Path(message["executable"]))
    require(finished == [True] and len(candidates) == 1,
            "need one successful build and exactly one emitted ordinary library test image")
    binary = candidates[0]
    return binary, binary_snapshot(binary, worktree, target, host)


def command(argv, **kwargs):
    return subprocess.run(argv, check=True, **kwargs)


def git(repo, *args, **kwargs):
    return command(
        ["git", "--no-replace-objects", "-c", "core.autocrlf=false", "-C", str(repo), *args],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        **kwargs,
    ).stdout


def authenticated_file(root, row):
    path = root / row["path"]
    require(stat.S_ISREG(path.lstat().st_mode), f"not a regular source file: {path}")
    data = path.read_bytes()
    require(len(data) == row["bytes"] and digest(data) == row["sha256"], f"source drift: {path}")
    return data


def old_source_proof(repo, worktree, image, proof):
    original = git(repo, "show", f"{image['base']}:{proof['path']}")
    require(len(original) == proof["bytes"] and digest(original) == proof["sha256"], "old whole-source mismatch")
    for name in ("driver", "frame"):
        row = proof[name]
        body = original[row["start"]:row["end"]]
        require(digest(body) == row["sha256"] and len(body.splitlines()) == row["lines"], f"old {name} mismatch")
    require(original[proof["frame"]["start"]:proof["frame"]["end"]].count(
        b"result: Option<(ResolvedType, OwnershipMode)>"
    ) == 5, "complete old owning frame lost a tuple")
    binding = json.loads((worktree / proof["observer_inverse"]).read_text())
    row = binding["rows"][proof["path"]]
    current = (worktree / proof["path"]).read_bytes()
    require(len(current) == row["length"] and digest(current) == row["sha256"], "old observed source drift")
    for edit in reversed(row["edits"]):
        start, end = edit["start"], edit["end"]
        require(current[start:end] == edit["current"].encode(), "old observer inverse changed bytes")
        current = current[:start] + edit["before"].encode() + current[end:]
    require(current == original, "observer patch changed the complete old driver")


def prepare(repo, path, image, manifest, fetch_missing):
    try:
        actual = git(repo, "rev-parse", "--verify", f"{image['base']}^{{commit}}").decode().strip()
    except subprocess.CalledProcessError:
        require(fetch_missing, f"missing public base {image['base']}; provision it or use --fetch-missing")
        git(repo, "fetch", "--no-write-fetch-head", "--no-tags", "origin", image["base"])
        actual = git(repo, "rev-parse", f"{image['base']}^{{commit}}").decode().strip()
    require(actual == image["base"], "base commit substitution")
    require(git(repo, "rev-parse", f"{actual}^{{tree}}").decode().strip() == image["base_tree"], "base tree substitution")
    patch = authenticated_file(PACKAGE, image["patch"])
    git(repo, "worktree", "add", "--detach", str(path), actual)
    git(path, "apply", "--index", "--whitespace=nowarn", "-", input=patch)
    verify_image(path, image)
    if image["name"] == "old":
        old_source_proof(repo, path, image, manifest["old_source_proof"])


def verify_image(path, image):
    require(git(path, "write-tree").decode().strip() == image["patched_tree"], "patched index tree mismatch")
    git(path, "diff", "--quiet")
    for source in image["sources"]:
        authenticated_file(path, source)


def read_records(log, image, manifest):
    records = []
    summary = None
    with log.open(encoding="utf-8") as stream:
        for line in stream:
            if MARKER in line:
                require(line.count(MARKER) == 1, "ambiguous record prefix")
                require(len(records) < manifest["records_per_image"], "too many matched records")
                records.append(json.loads(line.partition(MARKER)[2]))
            found = re.search(r"test result: ok\. (\d+) passed; (\d+) failed;", line)
            if found:
                summary = tuple(map(int, found.groups()))
    require(summary == (image["tests"], 0), "selected test count/status mismatch")
    require(len(records) == manifest["records_per_image"], "need exactly 66 complete matched records")
    groups = {}
    for row in records:
        require(row["image_base"] == image["record_base"], "record image substitution")
        require(row["owning_immediate_old"] == (image["name"] == "old"), "old/current role substitution")
        for key, expected in image["record_bindings"].items():
            require(row[key] == expected, f"record source identity mismatch: {key}")
        require(row["required_rust"] == manifest["rust"], "record toolchain contract mismatch")
        source = row["source"].encode()
        require(digest(source) == row["source_sha256"] and len(source) == row["source_bytes"], "authored source drift")
        require(row["live"] == 0 and row["allocated"] == row["freed"], "owned compiler Drop incomplete")
        require(row["paid"] <= row["limit"], "ledger exceeded selected bound")
        require(row["application_admission_proven"] is False and row["matched_old_image_required"] is True,
                "observation cannot claim application admission or erase pending matched proof")
        for error in row["diagnostics"]:
            require(set(error) == {"code", "severity", "message", "path", "span", "help"}, "incomplete six-field diagnostic")
        cases = groups.setdefault(row["source_sha256"], {})
        require(row["case"] not in cases, "duplicate source/case record")
        cases[row["case"]] = row
    require(len(groups) == manifest["fixtures"], "fixture count mismatch")
    for cases in groups.values():
        require(set(cases) == {"full", "exact", "short", "zero", "floor", "hostile"}, "incomplete case model")
        full = cases["full"]
        require(full["limit"] == 64 << 20 and full["floor"] == 0 and not full["attack"], "baseline drift")
        graph = full["accepted_graph"]
        require(isinstance(graph, str), "missing full accepted semantic graph")
        require(digest(graph.encode()) == full["accepted_graph_sha256"] and len(graph.encode()) == full["accepted_graph_bytes"], "graph bytes mismatch")
        json.loads(graph)
        require(full["frame_size"] > 0 and full["frame_align"] > 0, "unobserved complete enum layout")
        require(full["frame_size"] % full["frame_align"] == 0, "invalid enum alignment")
        require(full["requested_capacities"] == full["acquired_capacities"], "successful full backing incomplete")
        for case, row in cases.items():
            require(row["source"] == full["source"] and row["accepted_graph_sha256"] == full["accepted_graph_sha256"], "fixture graph/source changed between cases")
            require(row["accepted_graph_bytes"] == full["accepted_graph_bytes"], "graph length drift")
            require(row["attack"] == (case == "hostile"), "hostile case substitution")
            if case in ("full", "exact"):
                require(not row["diagnostics"] and not row["refused"] and row["first_refusal"] is None, "accepted whole caller failed")
            elif case == "hostile":
                require(row["diagnostics"] and not row["refused"] and row["first_refusal"] is None, "hostile six-field control failed")
            else:
                require(row["diagnostics"] and row["refused"] and row["first_refusal"] is not None, "refusal control incomplete")
                require(set(row["first_refusal"]) == {"requested", "remaining", "floor", "stage"}, "first receipt incomplete")
        require(cases["exact"]["limit"] == full["paid"] and cases["exact"]["paid"] == full["paid"], "exact invoice drift")
        require(cases["short"]["limit"] == full["paid"] - 1 and cases["short"]["floor"] == 0, "short boundary drift")
        require(cases["zero"]["limit"] == 0 and cases["zero"]["floor"] == 0, "zero boundary drift")
        require(cases["floor"]["limit"] == full["paid"] and cases["floor"]["floor"] == 1, "floor boundary drift")
    return groups


def compare(old, current):
    require(set(old) == set(current), "old/current authored fixture sets differ")
    for source, old_cases in old.items():
        new_cases = current[source]
        require(old_cases["full"]["accepted_graph"] == new_cases["full"]["accepted_graph"], "complete accepted semantic graphs differ")
        for case in ("full", "exact", "hostile"):
            require(old_cases[case]["diagnostics"] == new_cases[case]["diagnostics"], "six-field old/current diagnostics differ")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--scratch-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--fetch-missing", action="store_true")
    parser.add_argument("--fetch-dependencies", action="store_true",
                        help="explicitly provision locked Cargo dependencies before each offline test invocation")
    args = parser.parse_args()
    repo = args.repo.resolve(strict=True)
    scratch = args.scratch_root.resolve(strict=True)
    require(scratch.is_dir(), "scratch root must already exist")
    args.output.mkdir(mode=0o700)
    manifest_bytes = (PACKAGE / "manifest.json").read_bytes()
    manifest = json.loads(manifest_bytes)
    require(manifest["schema"] == SCHEMA, "runner manifest schema mismatch")
    require(git(repo, "remote", "get-url", "origin").decode().strip() in manifest["origins"], "unexpected origin")
    flags = os.O_CREAT | os.O_RDWR | os.O_CLOEXEC | os.O_NOFOLLOW
    lock = os.open("/tmp/semaprax-heavy-job.lock", flags, 0o600)
    created = []
    session = None
    result = {"schema": SCHEMA, "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "runner_sha256": digest(Path(__file__).read_bytes()), "manifest_sha256": digest(manifest_bytes),
              "host": platform.platform(), "host_lock": "/tmp/semaprax-heavy-job.lock",
              "available_physical_pages_bytes_before_jobs": available_memory(),
              "dependency_fetch_opted_in": args.fetch_dependencies,
              "application_admission_proven": False, "opt904_only_total_invoice_savings_proven": False}
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        rustc = command(["rustup", "run", manifest["rust"], "rustc", "--version", "--verbose"], stdout=subprocess.PIPE).stdout.decode()
        require(rustc.splitlines()[0].startswith(f"rustc {manifest['rust']} "), "actual compiler version mismatch")
        host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
        result["rustc_verbose"] = rustc
        result["cargo_verbose"] = command(["rustup", "run", manifest["rust"], "cargo", "--version", "--verbose"], stdout=subprocess.PIPE).stdout.decode()
        session = Path(tempfile.mkdtemp(prefix="opt904-actual-images-", dir=scratch))
        groups = {}
        for image in manifest["images"]:
            worktree = session / image["name"]
            created.append(worktree)
            prepare(repo, worktree, image, manifest, args.fetch_missing)
            env = os.environ.copy()
            for key in ("RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_ENCODED_RUSTFLAGS"):
                env.pop(key, None)
            env.update(manifest["environment"])
            env.update({"RUSTUP_TOOLCHAIN": manifest["rust"], "CARGO_BUILD_TARGET": host,
                        "CARGO_TARGET_DIR": str(worktree / "target" / "opt904-private")})
            image_result = {"base": image["base"], "base_tree": image["base_tree"],
                            "patched_tree": image["patched_tree"], "patch_sha256": image["patch"]["sha256"]}
            result[image["name"]] = image_result
            if args.fetch_dependencies:
                fetch_env = env.copy()
                fetch_env["CARGO_NET_OFFLINE"] = "false"
                fetch_log = args.output / f"{image['name']}-dependency-fetch.log"
                image_result["dependency_fetch"] = logged_process(
                    ["rustup", "run", manifest["rust"], "cargo", "fetch", "--locked", "--target", host],
                    worktree, fetch_env, fetch_log, lock,
                )
                require(image_result["dependency_fetch"]["status"] == 0,
                        f"{image['name']} locked dependency provisioning failed; separate log retained")
            build_log = args.output / f"{image['name']}-build.log"
            argv = ["rustup", "run", manifest["rust"], "cargo", "test", "--locked", "--offline", "-p", "semaprax",
                    "--lib", "--no-run", "--message-format=json"]
            image_result["build"] = logged_process(argv, worktree, env, build_log, lock)
            require(image_result["build"]["status"] == 0,
                    f"{image['name']} library test build failed; complete build log retained")
            target = Path(env["CARGO_TARGET_DIR"])
            binary, snapshot = emitted_library_test(build_log, worktree, target, host)
            image_result["executed_test_image"] = snapshot
            log = args.output / f"{image['name']}-test.log"
            require(binary_snapshot(binary, worktree, target, host) == snapshot, "test executable drift before execution")
            image_result.update(logged_process(
                [str(binary), manifest["selector"], "--test-threads=1", "--nocapture"],
                worktree, env, log, lock,
            ))
            require(binary_snapshot(binary, worktree, target, host) == snapshot, "test executable drift after execution")
            require(image_result["status"] == 0, f"{image['name']} selected tests failed; complete log retained")
            verify_image(worktree, image)
            groups[image["name"]] = read_records(log, image, manifest)
            git(repo, "worktree", "remove", "--force", str(worktree))
            created.remove(worktree)
            image_result["worktree_and_private_target_removed"] = True
            # Release this image's build storage before preparing the next image.
        compare(groups["old"], groups["current"])
        result["paired_observation_complete"] = True
        result["records_per_image"] = manifest["records_per_image"]
        result["fixture_sources"] = sorted(groups["old"])
        result["accepted_graph_sha256_by_source"] = {
            source: cases["full"]["accepted_graph_sha256"] for source, cases in groups["old"].items()
        }
        result["six_field_diagnostic_cases_compared"] = ["full", "exact", "hostile"]
        result["old_later_borrowed_witness_runtime_suite_qualified"] = False
    except BaseException as error:
        result["paired_observation_complete"] = False
        result["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        cleanup = []
        for worktree in reversed(created):
            if worktree.exists():
                try:
                    git(repo, "worktree", "remove", "--force", str(worktree))
                except subprocess.CalledProcessError:
                    cleanup.append(str(worktree))
        if session is not None and not cleanup:
            session.rmdir()
        result["cleanup_remaining"] = cleanup
        result["finished_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        (args.output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        os.close(lock)


if __name__ == "__main__":
    main()
