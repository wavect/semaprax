"""Pinned provider-only CPython and committed source projection; zero model calls."""
from __future__ import annotations
import argparse
import gzip
import hashlib
import io
import json
import os
import pathlib
import re
import subprocess
import tarfile
import tempfile
from . import pilot_protocol as p
from .pilot_linux_provider import checked_root
import runnable_v3_extraction as extraction

PYTHON_NAME = "cpython-3.12.14+20260901-aarch64-unknown-linux-gnu-install_only_stripped.tar.gz"
PYTHON_SHA256 = "577b4bec0793ad1ff0cbff9adbd0df078eddde38a4c41bf5d83ad381a85ee39d"
PYTHON_BYTES = 29199399
PYTHON_URL = "https://github.com/astral-sh/python-build-standalone/releases/download/20260901/" + PYTHON_NAME.replace("+", "%2B")
GUEST_PYTHON = "/opt/claude/python/bin/python3.12"


def selected(name):
    return name == "python/bin/python3.12" or name.startswith("python/lib/python3.12/") or name in (
        "python/lib/libpython3.12.so", "python/lib/libpython3.12.so.1.0")


def materialize_python(data, destination):
    """Caller authenticates compressed bytes first; no extractall/path repair."""
    destination = pathlib.Path(destination)
    destination.mkdir(mode=0o700)
    decoded = extraction.LimitedDecoded(gzip.GzipFile(fileobj=io.BytesIO(data)), 128 * 1024 * 1024)
    seen, selected_names, inventory, links = set(), set(), [], []
    total = 0
    try:
        with tarfile.open(fileobj=decoded, mode="r|", tarinfo=extraction.BoundedInfo) as archive:
            for index, member in enumerate(archive):
                parts = member.name.split("/")
                if (index >= 10000 or len(member.name.encode()) > 4096 or member.name in seen
                        or parts[0] != "python" or any(x in ("", ".", "..") for x in parts)):
                    raise ValueError("python_archive_path_refused")
                seen.add(member.name)
                if not selected(member.name) or member.isdir():
                    continue
                name = "/".join(parts[1:])
                if name.casefold() in selected_names:
                    raise ValueError("python_archive_case_collision")
                selected_names.add(name.casefold())
                path = destination / name
                path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                if member.issym():
                    if name != "lib/libpython3.12.so" or member.linkname != "libpython3.12.so.1.0":
                        raise ValueError("python_archive_link_refused")
                    links.append((path, member.linkname))
                    inventory.append({"path": name, "symlink": member.linkname})
                    continue
                if not member.isreg() or member.size > 64 * 1024 * 1024:
                    raise ValueError("python_archive_type_or_size_refused")
                total += member.size
                if total > 96 * 1024 * 1024:
                    raise ValueError("python_runtime_capacity_refused")
                with archive.extractfile(member) as source, path.open("xb") as target:
                    digest = hashlib.sha256()
                    copied = 0
                    while chunk := source.read(65536):
                        copied += len(chunk)
                        digest.update(chunk)
                        target.write(chunk)
                if copied != member.size:
                    raise ValueError("python_archive_truncated")
                mode = 0o500 if member.mode & 0o111 else 0o400
                path.chmod(mode)
                inventory.append({"path": name, "bytes": copied, "sha256": digest.hexdigest(), "mode": mode})
        while decoded.read(65536):
            pass
    finally:
        decoded.stream.close()
    interpreter = destination / "bin/python3.12"
    with interpreter.open("rb") as source:
        header = source.read(64)
    if header[:6] != b"\x7fELF\x02\x01" or header[18:20] != b"\xb7\x00":
        raise ValueError("python_interpreter_arch_refused")
    for path, target in links:
        if not (path.parent / target).is_file():
            raise ValueError("python_library_target_missing")
        path.symlink_to(target)
    for directory, _, _ in os.walk(destination, topdown=False):
        pathlib.Path(directory).chmod(0o500)
    return sorted(inventory, key=lambda row: row["path"])


def stage_python(archive_path, root, receipt_digest):
    root, _, cli, environment = checked_root(root, receipt_digest)
    data = p.provenance.read_regular(pathlib.Path(archive_path), 40 * 1024 * 1024)
    if len(data) != PYTHON_BYTES or p.digest(data) != PYTHON_SHA256:
        raise ValueError("python_archive_identity_refused")
    inventory = materialize_python(data, root / "public/python")
    command = [str(cli), "exec", "semaprax-issue332-provider", GUEST_PYTHON, "-I", "-S", "-B", "-c",
               "import sys,platform,json;print(json.dumps({'version':platform.python_version(),'machine':platform.machine(),'isolated':sys.flags.isolated,'no_site':sys.flags.no_site,'no_bytecode':sys.dont_write_bytecode}))"]
    result = subprocess.run(command, env=environment, cwd=root / "scratch", capture_output=True, timeout=15, check=True)
    observed = p.strict_json(result.stdout)
    if observed != {"version": "3.12.14", "machine": "aarch64", "isolated": 1, "no_site": 1, "no_bytecode": True}:
        raise ValueError("python_guest_identity_refused")
    receipt = {"schema": "benchmark.cross_language.linux_provider_python.v1", "archive_url": PYTHON_URL,
               "archive_sha256": PYTHON_SHA256, "archive_bytes": PYTHON_BYTES, "inventory": inventory,
               "inventory_sha256": p.digest(p.canonical(inventory)), "guest_executable": GUEST_PYTHON,
               "observed": observed, "model_dispatches": 0,
               "selection": "exact interpreter, libpython and stdlib; unused share/man/terminfo omitted"}
    with (root / "provider-runtime.json").open("xb") as output:
        output.write(p.canonical(receipt))
    return receipt


def materialize_source(archive_path, destination):
    """Only committed regular files/directories; refuse links and Git metadata."""
    destination = pathlib.Path(destination)
    destination.mkdir(mode=0o700)
    seen, folded, inventory, total = set(), set(), [], 0
    with tarfile.open(archive_path, "r:", tarinfo=extraction.BoundedInfo) as archive:
        for index, member in enumerate(archive):
            parts = member.name.split("/")
            if (index >= 20000 or len(member.name.encode()) > 4096 or member.name in seen
                    or any(x in ("", ".", "..", ".git") for x in parts)
                    or pathlib.PurePosixPath(member.name).is_absolute()):
                raise ValueError("source_archive_path_refused")
            seen.add(member.name)
            if member.isdir():
                continue
            if member.name.casefold() in folded or not member.isreg() or member.size > 32 * 1024 * 1024:
                raise ValueError("source_archive_type_refused")
            folded.add(member.name.casefold())
            total += member.size
            if total > 128 * 1024 * 1024:
                raise ValueError("source_archive_capacity_refused")
            path = destination / member.name
            path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            with archive.extractfile(member) as source, path.open("xb") as target:
                data = source.read(member.size + 1)
                if len(data) != member.size:
                    raise ValueError("source_archive_truncated")
                target.write(data)
            mode = 0o500 if member.mode & 0o111 else 0o400
            path.chmod(mode)
            inventory.append({"path": member.name, "bytes": len(data), "sha256": p.digest(data), "mode": mode})
    for directory, _, _ in os.walk(destination, topdown=False):
        pathlib.Path(directory).chmod(0o500)
    return sorted(inventory, key=lambda row: row["path"])


def stage_source(repository, root, receipt_digest, expected_head):
    root, _, _, _ = checked_root(root, receipt_digest)
    repository = pathlib.Path(repository)
    if repository != repository.resolve() or not re.fullmatch(r"[0-9a-f]{40}", expected_head):
        raise ValueError("source_projection_identity_refused")
    env = {"PATH": "/usr/bin:/bin", "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
    def git(*args):
        return subprocess.run(["/usr/bin/git", "-C", str(repository), *args], env=env, capture_output=True, timeout=30, check=True).stdout
    if git("rev-parse", "HEAD").decode().strip() != expected_head or git("status", "--porcelain"):
        raise ValueError("committed_clean_source_required")
    with tempfile.TemporaryDirectory(prefix="pilot-source-", dir=root / "scratch") as temporary:
        archive = pathlib.Path(temporary) / "source.tar"
        git("archive", "--format=tar", "--output=" + str(archive), expected_head)
        size, sha = p.provenance.file_digest(archive, 160 * 1024 * 1024)
        inventory = materialize_source(archive, root / "public/source")
    receipt = {"schema": "benchmark.cross_language.linux_provider_source.v1", "controller_git_head": expected_head,
               "guest_git_observation": False, "archive_sha256": sha, "archive_bytes": size,
               "inventory_sha256": p.digest(p.canonical(inventory)), "inventory": inventory,
               "git_metadata_copied": False, "guest_source": "/opt/claude/source", "model_dispatches": 0}
    with (root / "source-receipt.json").open("xb") as output:
        output.write(p.canonical(receipt))
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True)
    parser.add_argument("--receipt-sha256", required=True)
    commands = parser.add_subparsers(dest="command", required=True)
    python = commands.add_parser("python")
    python.add_argument("--archive", required=True)
    source = commands.add_parser("source")
    source.add_argument("--repository", required=True)
    source.add_argument("--head", required=True)
    args = parser.parse_args()
    receipt = (stage_python(args.archive, args.root, args.receipt_sha256) if args.command == "python" else
               stage_source(args.repository, args.root, args.receipt_sha256, args.head))
    print(json.dumps({"receipt_sha256": p.digest(p.canonical(receipt)), "files": len(receipt["inventory"]), "model_dispatches": 0}))


if __name__ == "__main__":
    main()
