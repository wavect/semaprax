#!/usr/bin/env python3
"""Offline, operator-selected reference-service development/OCI packaging.

This is not `semaprax build --target oci` and does not establish executable
provenance. The checker is explicitly trusted executable authority; the runtime
is copied, never executed. See docs/REFERENCE-SERVICE-HOST-V1.md.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import stat
import struct
import subprocess
import tarfile
import tempfile

PROJECT_FILES = ("semaprax.toml", "src/app.spx", "src/core.spx", "src/tests.spx")
MAX_SOURCE = 4 * 1024 * 1024
MAX_EXECUTABLE = 512 * 1024 * 1024


def digest(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def read_regular(path, limit):
    # No recursive traversal, symlink following at the leaf, or arbitrary files
    # from the project directory. The copied project is checked independently.
    if not hasattr(os, "O_NOFOLLOW"):
        raise ValueError("packaging host needs no-follow regular-file reads")
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as stream:
        if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
            raise ValueError("input must be a regular file")
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError("input exceeds package limit")
    return data


def linux_static_elf(data):
    """Bounded format admission, not a proof of executable behavior/provenance."""
    if len(data) < 64 or data[:7] != b"\x7fELF\x02\x01\x01" or data[7] not in (0, 3):
        raise ValueError("OCI needs a little-endian 64-bit static Linux ELF")
    kind, machine, version = struct.unpack_from("<HHI", data, 16)
    phoff = struct.unpack_from("<Q", data, 32)[0]
    ehsize, phsize, count = struct.unpack_from("<HHH", data, 52)
    if (kind != 2 or version != 1 or ehsize != 64 or phsize != 56
            or not 1 <= count <= 128 or phoff < 64
            or phoff + count * phsize > len(data) or machine not in (62, 183)):
        raise ValueError("unsupported ELF header/program table (static ET_EXEC required)")
    executable_load = False
    for i in range(count):
        offset = phoff + i * phsize
        segment, flags, file_offset, _, _, file_size, memory_size, _ = struct.unpack_from("<IIQQQQQQ", data, offset)
        if segment in (2, 3):
            raise ValueError("dynamic/interpreted ELF refused: no loader or shared libraries packaged")
        if file_offset + file_size > len(data) or file_size > memory_size:
            raise ValueError("invalid ELF segment extent")
        executable_load |= segment == 1 and bool(flags & 1) and file_size > 0
    if not executable_load:
        raise ValueError("ELF needs an executable load segment")
    return "amd64" if machine == 62 else "arm64"


def admit_executable(data, mode):
    if mode == "oci" or platform.system() == "Linux":
        arch = linux_static_elf(data)
        if mode == "development" and arch != {"x86_64": "amd64", "aarch64": "arm64"}.get(platform.machine()):
            raise ValueError("development executable architecture differs from host")
        return "linux", arch
    if platform.system() == "Darwin":
        if len(data) < 32 or data[:4] != b"\xcf\xfa\xed\xfe":
            raise ValueError("development package on macOS needs a thin 64-bit Mach-O")
        cpu, _, filetype = struct.unpack_from("<III", data, 4)
        arch = {0x01000007: "amd64", 0x0100000C: "arm64"}.get(cpu)
        if filetype != 2 or arch != {"x86_64": "amd64", "arm64": "arm64"}.get(platform.machine()):
            raise ValueError("development executable type/architecture differs from host")
        return "darwin", arch
    raise ValueError("development packaging supports Linux static ELF and macOS thin Mach-O only")


def layer_bytes(files):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for name in ("bin", "bundle", "outbound", "secrets", "service", "service/src", "state"):
            info = tarfile.TarInfo(name)
            info.type, info.mode, info.uid, info.gid, info.mtime = tarfile.DIRTYPE, 0o755, 0, 0, 0
            archive.addfile(info)
        for name, (data, mode) in sorted(files.items()):
            info = tarfile.TarInfo(name)
            info.size, info.mode, info.uid, info.gid, info.mtime = len(data), mode, 0, 0, 0
            archive.addfile(info, io.BytesIO(data))
    return output.getvalue()


def write_oci(output, files, arch):
    blobs = output / "blobs" / "sha256"
    blobs.mkdir(parents=True)

    def blob(data, media):
        sha = digest(data)
        (blobs / sha[7:]).write_bytes(data)
        return {"mediaType": media, "digest": sha, "size": len(data)}

    layer = blob(layer_bytes(files), "application/vnd.oci.image.layer.v1.tar")
    config = blob(encoded({
        "architecture": arch, "os": "linux",
        "config": {"Entrypoint": [
            "/bin/semaprax-reference-service", "serve",
            "--project", "/service", "--config", "/service/service.config.json",
            "--state-dir", "/state", "--outbound-dir", "/outbound",
            "--secrets-dir", "/secrets", "--bundle-dir", "/bundle", "--port", "8080",
        ], "User": "65532:65532",
                   "WorkingDir": "/service"},
        "rootfs": {"type": "layers", "diff_ids": [layer["digest"]]},
    }), "application/vnd.oci.image.config.v1+json")
    manifest = blob(encoded({"schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": config, "layers": [layer]}), "application/vnd.oci.image.manifest.v1+json")
    (output / "oci-layout").write_bytes(encoded({"imageLayoutVersion": "1.0.0"}))
    (output / "index.json").write_bytes(encoded({"schemaVersion": 2, "manifests": [manifest]}))


def package(args):
    executable = read_regular(args.executable, MAX_EXECUTABLE)
    if digest(executable) != args.executable_sha256:
        raise ValueError("executable digest differs from explicit expected digest")
    operating_system, arch = admit_executable(executable, args.format)
    files = {"bin/semaprax-reference-service": (executable, 0o755)}
    for name in PROJECT_FILES:
        files["service/" + name] = (read_regular(args.project / name, MAX_SOURCE), 0o644)
    files["service/service.config.json"] = (read_regular(args.config, 64 * 1024), 0o644)
    checker = args.checker.resolve(strict=True)
    # Stage only bounded copied bytes, then check those exact bytes. Source
    # directory drift cannot substitute unchecked content into this package.
    with tempfile.TemporaryDirectory(prefix="semaprax-service-check-") as temporary:
        staged = Path(temporary)
        for name, (data, _) in files.items():
            if name.startswith("service/"):
                target = staged / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
        subprocess.run([str(checker), "check-package", "--project", str(staged / "service"),
                        "--config", str(staged / "service/service.config.json")],
                       check=True, timeout=120, stdin=subprocess.DEVNULL)
    # Caller owns the destination parent. Exclusive mkdir refuses overwrite;
    # receipt is written last. No automatic cleanup of a partial publication.
    args.output.mkdir()
    if args.format == "oci":
        write_oci(args.output, files, arch)
    else:
        for name, (data, mode) in sorted(files.items()):
            target = args.output / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            target.chmod(mode)
    receipt = {"schema": "semaprax.reference-service.package.v1",
               "format": args.format, "os": operating_system, "architecture": arch,
               "files": [{"path": name, "digest": digest(data), "bytes": len(data)}
                         for name, (data, _) in sorted(files.items())],
               "nonclaims": ["unsigned", "executable_provenance_not_established",
                             "runtime_execution_not_performed", "no_secrets_or_runtime_grants"]}
    (args.output / "service-package.json").write_bytes(encoded(receipt))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("executable", "checker", "project", "config", "output"):
        parser.add_argument("--" + flag, required=True, type=Path)
    parser.add_argument("--executable-sha256", required=True)
    parser.add_argument("--format", required=True, choices=("development", "oci"))
    args = parser.parse_args()
    try:
        package(args)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(2, f"refused: {error}\n")


if __name__ == "__main__":
    main()
