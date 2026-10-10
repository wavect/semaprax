"""Read-only completed specimen byte checks, never acceptance or receipt authority.

The operator owns the isolated evidence tree. This check detects missing, stale
or edited inputs before qualification; it does not freeze paths or prove that a
compiler/provider executed. The acceptance harness must still check the live tree.
"""
from __future__ import annotations
import json
import os
from pathlib import Path
import re
import stat
import subprocess


def _snapshot(root: Path, value: object, directory: str, api) -> list[dict]:
    expected = root / directory / "snapshot.json"
    if (not isinstance(value, dict) or set(value) != {"path", "sha256"}
            or value["path"] != str(expected)
            or not isinstance(value["sha256"], str)
            or not re.fullmatch(r"[0-9a-f]{64}", value["sha256"])):
        raise ValueError(f"{directory} snapshot selection differs")
    encoded = api.read_regular(expected, api.provenance.MAX_INPUT_RECEIPT_BYTES)
    header = json.loads(encoded)
    rows = header.get("input_files") if isinstance(header, dict) else None
    if not isinstance(rows, list):
        raise ValueError(f"{directory} snapshot inventory differs")
    total = 0
    for row in rows:
        size = row.get("bytes") if isinstance(row, dict) else None
        if isinstance(size, bool) or not isinstance(size, int) or size < 0:
            raise ValueError(f"{directory} snapshot byte count differs")
        total += size
        if total > api.MAX_BYTES:
            raise ValueError(f"{directory} snapshot exceeds setup byte budget")
    rows = api.provenance.validate_input_snapshot(expected, value["sha256"])
    for row in rows:
        # Check every directory component, including the retained snapshot root.
        data = api.read_regular(expected.parent / "inputs" / row["path"])
        if len(data) != row["bytes"] or api.sha(data) != row["sha256"]:
            raise ValueError(f"{directory} retained input differs: {row['path']}")
    return rows


def _closed_tree(project: Path, paths: list[str], application: str, api) -> None:
    # Only the explicit operator build outputs and Project lock may coexist
    # with installation inputs. Invocation-owned frontend caches are in memory;
    # no arbitrary .cache/target tree grants an unlisted source exception.
    allowed = set(paths) | {"semaprax.lock", f"dist/{application}", f"dist/{application}.c"}
    directories = {"dist"} | {str(Path(path).parent) for path in allowed if "/" in path}
    root_fd = api.directory_fd(project)
    count = 0

    def walk(fd, prefix):
        nonlocal count
        for name in sorted(os.listdir(fd)):
            count += 1
            if count > 1024:
                raise ValueError("installed tree exceeds closed entry budget")
            relative = prefix + name
            info = os.stat(name, dir_fd=fd, follow_symlinks=False)
            if stat.S_ISDIR(info.st_mode):
                if relative not in directories:
                    raise ValueError(f"unlisted installed directory: {relative}")
                child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                try:
                    walk(child, relative + "/")
                finally:
                    os.close(child)
            elif not stat.S_ISREG(info.st_mode):
                raise ValueError(f"installed path must be regular: {relative}")
            elif relative not in allowed:
                raise ValueError(f"unlisted installed file: {relative}")

    try:
        walk(root_fd, "")
    finally:
        os.close(root_fd)


def verify(args, *, api) -> Path:
    root, repo = args.output.absolute(), args.repo.absolute()
    for directory in (root, repo):
        fd = api.directory_fd(directory)
        api.os.close(fd)
    source, binary = args.compiler_source, args.compiler_sha256
    if (not isinstance(source, str) or not re.fullmatch(r"[0-9a-f]{40}", source)
            or not isinstance(binary, str) or not re.fullmatch(r"[0-9a-f]{64}", binary)):
        raise ValueError("exact source commit and lowercase binary SHA-256 are required")
    if (subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"]).decode().strip() != source
            or api.digest_regular(args.compiler.absolute()) != binary):
        raise ValueError("compiler source/binary changed")
    binding = api.verify_build_receipt(args.compiler_build_receipt, source, binary)
    result = json.loads(api.read_regular(root / "setup-result.json", 1024 * 1024))
    done = json.loads(api.read_regular(root / "completion-receipt.json", 1024 * 1024))
    if (not isinstance(result, dict) or not isinstance(done, dict)
            or result.get("schema") != "semaprax.typed-application-setup.v1"
            or result.get("status") != "checked_source_runtime_qualification_pending"
            or result.get("application") != args.application or result.get("profile") != api.PROFILE
            or result.get("compiler_source_commit") != source
            or result.get("compiler_binary_sha256") != binary
            or result.get("compiler_build_receipt") != binding
            or result.get("runtime_qualification") is not None
            or done.get("schema") != "semaprax.typed-application-completion.v1"
            or done.get("project") != "installed-project"
            or done.get("runtime_qualification") is not None
            or (root / "completion-failure.json").exists()
            or (root / "completion-failure.json").is_symlink()):
        raise ValueError("setup source/completion receipt differs; repeat setup in a new output directory")
    selected = _snapshot(root, result.get("selected_input_snapshot"), "selected-inputs", api)
    authored = _snapshot(root, result.get("authored_input_snapshot"), "authored-inputs", api)
    installed = _snapshot(root, result.get("installed_input_snapshot"), "installed-inputs", api)
    authored_paths = api.selected_paths(args.application)
    if [row["path"] for row in authored] != authored_paths:
        raise ValueError("authored source inventory differs from the closed specimen")
    example = "examples/" + api.EXAMPLES[args.application]["directory"] + "/"
    selected_by_path = {row["path"]: row for row in selected}
    for row in selected:
        path = row["path"]
        mode = subprocess.check_output(["git", "-C", str(repo), "ls-tree", source, "--", path]).decode().split()
        committed = subprocess.check_output(["git", "-C", str(repo), "show", f"{source}:{path}"])
        if (not mode or mode[0] not in ("100644", "100755")
                or committed != api.read_regular(repo / path) or api.sha(committed) != row["sha256"]):
            raise ValueError(f"selected committed setup source changed: {path}")
    for row in authored:
        original = selected_by_path.get(example + row["path"])
        if original is None or (original["sha256"], original["bytes"]) != (row["sha256"], row["bytes"]):
            raise ValueError(f"authored bootstrap differs from original source: {row['path']}")
    manifest_path = root / "authored-inputs/inputs/semaprax.toml"
    manifest = api.validate_manifest(args.application, api.read_regular(manifest_path))
    registry = api.dependencies.parse_registry(api.read_regular(repo / "src/project/standard_dependencies.rs").decode())
    packages = json.loads(api.read_regular(repo / "std/packages.json"))["packages"]
    directories = {row["module"]: row["directory"] for row in packages}
    dependencies = api.dependencies.closure(manifest, registry)
    required = {example + path for path in authored_paths} | {
        "benchmarks/typed_application_setup.py", "benchmarks/compiler_output_provenance.py",
        "benchmarks/typed_application_setup_support/dependencies.py",
        "benchmarks/typed_application_setup_support/installation.py",
        "src/project/standard_dependencies.rs", "std/packages.json"}
    if len(directories) != len(packages) or set(directories) != set(registry):
        raise ValueError("package catalog differs from actual bundled registry")
    for name in dependencies:
        directory = directories[name]
        if not re.fullmatch(r"[a-z0-9-]+", directory):
            raise ValueError("invalid bundled package directory")
        package = registry[name]
        prefix = "std/" + directory + "/"
        included = package["source"]
        if not included.startswith(prefix):
            raise ValueError("bundled include differs from package catalog")
        api.dependencies.validate_package(package, json_manifest(repo / prefix / "semaprax.toml", api),
                                          included[len(prefix):])
        required.update((prefix + "semaprax.toml", included))
    if set(selected_by_path) != required or result.get("bundled_dependencies") != dependencies:
        raise ValueError("selected setup dependency closure differs")
    scripts = api.qualification_scaffolding(args.application)
    paths = sorted((set(authored_paths) - {"src/app.command.spx"}) | set(scripts))
    inventory = [{"path": row["path"], "sha256": row["sha256"]} for row in installed]
    if ([row["path"] for row in installed] != paths
            or inventory != result.get("installed_source_inventory") or inventory != done.get("source_inventory")):
        raise ValueError("installed source inventory differs from the closed specimen")
    project = root / "installed-project"
    _closed_tree(project, paths, args.application, api)
    generated = result.get("generated_outputs")
    if (not isinstance(generated, list)
            or [row.get("path") for row in generated if isinstance(row, dict)]
            != [row[0] for row in api.generator_jobs(args.application)]):
        raise ValueError("checked generator output inventory differs")
    generated_by_path = {row["path"]: row for row in generated}
    for index, (path, *_rest) in enumerate(api.generator_jobs(args.application)):
        first = api.read_regular(root / "generated" / f"{index}.spx", api.MAX_GENERATED_BYTES)
        replay = api.read_regular(root / "generated" / f"{index}.replay.spx", api.MAX_GENERATED_BYTES)
        row = generated_by_path[path]
        if not first or first != replay or (api.sha(first), len(first)) != (row.get("sha256"), row.get("bytes")):
            raise ValueError(f"retained generator output differs: {path}")
    originals = {row["path"]: row for row in authored}
    for row in installed:
        data = api.read_regular(project / row["path"])
        if api.sha(data) != row["sha256"] or len(data) != row["bytes"]:
            raise ValueError(f"installed reviewed source drift: {row['path']}")
        if row["path"] in scripts:
            if data != scripts[row["path"]]:
                raise ValueError("operator scaffolding differs")
        else:
            expected = (generated_by_path.get(row["path"])
                        or originals["src/app.command.spx" if row["path"] == "src/app.spx" else row["path"]])
            if (expected.get("sha256"), expected.get("bytes")) != (row["sha256"], row["bytes"]):
                raise ValueError(f"installed source differs from retained derivation: {row['path']}")
    return project


def json_manifest(path: Path, api) -> dict:
    return api.tomllib.loads(api.read_regular(path).decode())
