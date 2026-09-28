"""Fixed official-origin and source-subject admission for adapter v3."""
from __future__ import annotations
import base64
import hashlib
import json
import os
import pathlib
import stat
import struct
import subprocess
import runnable_adapter as v1

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE_MANIFEST = pathlib.Path(__file__).resolve().parent / "provenance/typescript-official-v3-source.json"
SOURCE_HASH = "c69695fd5f16917d745ef4c47c1d54438455d541cc4801046dd764e50a0fb83c"
SOURCE_COMMIT = "8e2a1c58324fb17308084e3cef149494259bd585"
NODE_HASH = "0047be0cfda922eb73876f9ef41de361c36b7654c884d13d9b783b0efd1db9aa"
NODE_BINARY_HASH = "53dc65febda99ecaafe692de5ec60efdc2f7bd4fb14d1ba8cd30dc2af103953f"
NODE_RECEIPT_HASH = "4d4dc7ec5755c7b034127c9f13ba83efe9869d2aa11c173d901a551da0f4649f"
TS_RECEIPT_HASH = "903e88bb6d16ca9419e14722378df6b66de31187f3534fb90a18e9ee2b37582e"
TS_INTEGRITY = "p1diW6TqL9L07nNxvRMM7hMMw4c5XOo/1ibL4aAIGmSAt9slTE1Xgw5KWuof2uTOvCg9BY7ZRi+GaF+7sfgPeQ=="
NODE_NAME = "node-v22.12.0-darwin-arm64.tar.xz"
TS_NAME = "typescript-5.8.3.tgz"
LIBRARIES = {
    "/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation",
    "/usr/lib/libSystem.B.dylib", "/usr/lib/libc++.1.dylib",
}
Error = v1.SnapshotError


def canonical(value):
    return (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n").encode("ascii")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def open_regular(path, limit):
    """Acquire every ancestor no-follow; caller owns the returned file fd."""
    path = pathlib.Path(path)
    if not path.is_absolute() or any(x in (".", "..") for x in path.parts):
        raise Error("invalid_authorized_path")
    fd = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
    try:
        for component in path.parts[1:-1]:
            nxt = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=fd)
            os.close(fd)
            fd = nxt
        leaf = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=fd)
        facts = os.fstat(leaf)
        if not stat.S_ISREG(facts.st_mode) or facts.st_size > limit:
            os.close(leaf)
            raise Error("regular_file_type_or_size_refused")
        return leaf, facts
    except OSError as error:
        raise Error("nofollow_acquisition_refused") from error
    finally:
        os.close(fd)


def _read_chunks(path, limit):
    fd, before = open_regular(path, limit)
    total = 0
    try:
        while True:
            chunk = os.read(fd, min(65536, limit + 1 - total))
            if not chunk:
                break
            total += len(chunk)
            if total > limit:
                raise Error("regular_file_changed_or_oversized")
            yield chunk
        after = os.fstat(fd)
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
            raise Error("regular_file_changed_or_oversized")
    finally:
        os.close(fd)


def read_regular(path, limit):
    return b"".join(_read_chunks(path, limit))


def file_digest(path, limit):
    hash_value = hashlib.sha256()
    total = 0
    for chunk in _read_chunks(path, limit):
        total += len(chunk)
        hash_value.update(chunk)
    return total, hash_value.hexdigest()


def approved_archives(directory):
    directory = pathlib.Path(directory)
    receipt = read_regular(directory / "node22.12.0-SHASUMS256.txt", 65536)
    metadata = read_regular(directory / "typescript5.8.3-registry.json", 65536)
    if digest(receipt) != NODE_RECEIPT_HASH or digest(metadata) != TS_RECEIPT_HASH:
        raise Error("official_receipt_identity_drifted")
    if f"{NODE_HASH}  {NODE_NAME}" not in receipt.decode().splitlines():
        raise Error("node_checksum_receipt_disagrees")
    ts = json.loads(metadata)
    dist = ts["dist"]
    if (ts["name"], ts["version"], dist["tarball"], dist["integrity"], dist["fileCount"], dist["unpackedSize"]) != (
        "typescript", "5.8.3", "https://registry.npmjs.org/typescript/-/typescript-5.8.3.tgz", "sha512-" + TS_INTEGRITY, 130, 22867703):
        raise Error("typescript_receipt_disagrees")
    node = read_regular(directory / NODE_NAME, 64 * 1024 * 1024)
    typescript = read_regular(directory / TS_NAME, 16 * 1024 * 1024)
    if digest(node) != NODE_HASH:
        raise Error("node_archive_identity_drifted")
    if base64.b64encode(hashlib.sha512(typescript).digest()).decode() != TS_INTEGRITY:
        raise Error("typescript_archive_identity_drifted")
    return node, typescript, {"node_receipt": receipt.decode(), "typescript_receipt": json.loads(metadata),
                              "node_archive_sha256": digest(node), "typescript_archive_sha256": digest(typescript)}


def source_snapshot(root=None):
    root = ROOT if root is None else pathlib.Path(root)
    manifest_bytes = read_regular(SOURCE_MANIFEST, 128 * 1024)
    if digest(manifest_bytes) != SOURCE_HASH:
        raise Error("approved_source_manifest_drifted")
    manifest = json.loads(manifest_bytes)
    if manifest["git_subject"] != SOURCE_COMMIT or len(manifest["comparison_inventory"]) != 182:
        raise Error("approved_source_manifest_invalid")
    contents = {}
    total = 0
    for row in manifest["files"]:
        data = read_regular(pathlib.Path(root) / row["path"], v1.MAX_SOURCE_FILE_BYTES)
        total += len(data)
        if total > v1.MAX_SOURCE_TOTAL_BYTES:
            raise Error("source_snapshot_exceeds_byte_bound")
        if len(data) != row["bytes"] or "sha256:" + digest(data) != row["sha256"]:
            raise Error("approved_source_input_drifted:" + row["path"])
        contents[row["path"]] = data
    return manifest, contents


def check_macho(data):
    if len(data) < 32:
        raise Error("node_macho_invalid")
    magic, cpu, _, _, count, size, _, _ = struct.unpack_from("<8I", data)
    if magic != 0xfeedfacf or cpu != 0x100000c or size > len(data) - 32 or count > 4096:
        raise Error("node_macho_invalid")
    offset = 32
    imports = []
    for _ in range(count):
        if offset + 8 > 32 + size:
            raise Error("node_macho_invalid")
        command, length = struct.unpack_from("<2I", data, offset)
        if length < 8 or offset + length > 32 + size:
            raise Error("node_macho_invalid")
        if command == 0x8000001c:
            raise Error("node_rpath_refused")
        if command in (0xc, 0x80000018, 0x8000001f, 0x20, 0x80000023):
            if length < 24:
                raise Error("node_macho_invalid")
            name_offset = struct.unpack_from("<I", data, offset + 8)[0]
            if name_offset < 24 or name_offset >= length:
                raise Error("node_macho_invalid")
            raw = data[offset + name_offset:offset + length].split(b"\0", 1)
            if len(raw) != 2:
                raise Error("node_macho_invalid")
            imports.append(raw[0].decode("ascii"))
        offset += length
    if offset != 32 + size or len(imports) != 3 or set(imports) != LIBRARIES:
        raise Error("node_dependency_identity_drifted")
    return imports


def host_identity():
    if os.uname().sysname != "Darwin" or os.uname().machine != "arm64":
        raise Error("official_profile_host_unavailable")
    # OS-protected host tool is explicit authority, not a caller search path.
    data = read_regular(pathlib.Path("/usr/bin/sw_vers"), 1024 * 1024)
    v1._admit_host_executable(pathlib.Path("/usr/bin/sw_vers"), "sha256:" + digest(data), "sw_vers")
    result = subprocess.run(["/usr/bin/sw_vers"], capture_output=True, env=dict(v1.CLOSED_ENVIRONMENT), timeout=5, check=False)
    rows = dict(line.split(":", 1) for line in result.stdout.decode().splitlines())
    if result.returncode or rows.get("ProductVersion", "").strip() != "26.5.1" or rows.get("BuildVersion", "").strip() != "25F80":
        raise Error("official_profile_host_unavailable")
    return {"platform": "darwin-arm64", "version": "26.5.1", "build": "25F80", "sw_vers_sha256": digest(data)}
