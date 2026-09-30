"""Authenticated, manually materialized bounded archive payloads."""
from __future__ import annotations
import gzip
import hashlib
import io
import lzma
import os
import pathlib
import tarfile
import runnable_adapter as v1
import runnable_v3_provenance as p


class LimitedDecoded:
    def __init__(self, stream, limit):
        self.stream, self.limit, self.total = stream, limit, 0

    def read(self, size=-1):
        if size < 0:
            size = 65536
        data = self.stream.read(min(size, self.limit + 1 - self.total))
        self.total += len(data)
        if self.total > self.limit:
            raise p.Error("archive_decoded_stream_exceeds_bound")
        return data


class BoundedInfo(tarfile.TarInfo):
    def _proc_pax(self, archive):
        if self.size > 1024 * 1024:
            raise p.Error("archive_extended_metadata_exceeds_bound")
        return super()._proc_pax(archive)

    def _proc_gnulong(self, archive):
        if self.size > 1024 * 1024:
            raise p.Error("archive_extended_metadata_exceeds_bound")
        return super()._proc_gnulong(archive)


def scan_archive(data, kind, *, decoded_limit=None, output_root=None):
    """Pure parser; callers must authenticate original compressed bytes first."""
    node = kind == "node"
    if kind not in ("node", "typescript"):
        raise p.Error("invalid_archive_kind")
    expanded_limit = decoded_limit if decoded_limit is not None else (512 if node else 40) * 1024 * 1024
    stream = lzma.LZMAFile(io.BytesIO(data)) if node else gzip.GzipFile(fileobj=io.BytesIO(data))
    limited = LimitedDecoded(stream, expanded_limit)
    files, names, count, total = {}, set(), 0, 0
    try:
        with tarfile.open(fileobj=limited, mode="r|", tarinfo=BoundedInfo) as archive:
            for member in archive:
                count += 1
                if count > (10000 if node else 512):
                    raise p.Error("archive_member_count_exceeds_bound")
                name = member.name
                path = pathlib.PurePosixPath(name)
                if (len(name.encode()) > 4096 or path.is_absolute() or not path.parts
                        or any(x in ("", ".", "..") for x in name.split("/")) or name in names):
                    raise p.Error("archive_member_path_refused")
                names.add(name)
                selected = name == "node-v22.12.0-darwin-arm64/bin/node" if node else member.isreg()
                expected_root = "node-v22.12.0-darwin-arm64" if node else "package"
                if path.parts[0] != expected_root:
                    raise p.Error("archive_member_path_refused")
                if node:
                    if not (member.isreg() or member.isdir() or member.issym()):
                        raise p.Error("archive_member_type_refused")
                elif not (member.isreg() or member.isdir()):
                    raise p.Error("archive_member_type_refused")
                if not selected:
                    continue
                if not member.isreg():
                    raise p.Error("archive_selected_member_not_regular")
                if member.size > (128 if node else 16) * 1024 * 1024:
                    raise p.Error("archive_member_size_exceeds_bound")
                total += member.size
                if not node and total > 32 * 1024 * 1024:
                    raise p.Error("archive_regular_bytes_exceed_bound")
                relative = "node" if node else "typescript/" + "/".join(path.parts[1:])
                with archive.extractfile(member) as file:
                    if output_root is None:
                        content = file.read(member.size + 1)
                        if len(content) != member.size:
                            raise p.Error("archive_member_truncated")
                        files[relative] = content
                    else:
                        destination = pathlib.Path(output_root) / relative
                        destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                        digest = hashlib.sha256()
                        written = 0
                        fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC, 0o600)
                        with os.fdopen(fd, "wb") as output:
                            while chunk := file.read(65536):
                                written += len(chunk)
                                digest.update(chunk)
                                output.write(chunk)
                        if written != member.size:
                            raise p.Error("archive_member_truncated")
                        mode = 0o500 if relative == "node" else 0o400
                        os.chmod(destination, mode)
                        files[relative] = {"path": relative, "bytes": written, "sha256": digest.hexdigest(), "mode": mode}
        # Tar iteration stops at end markers; count all padding/trailing decoded
        # bytes, including ignored payloads, before accepting the archive.
        while limited.read(65536):
            pass
    except (tarfile.TarError, EOFError, lzma.LZMAError, gzip.BadGzipFile, OSError) as error:
        raise p.Error("archive_decode_refused") from error
    finally:
        stream.close()
    if node and set(files) != {"node"}:
        raise p.Error("archive_selected_member_missing")
    if not node and (len(files) != 130 or total != 22867703):
        raise p.Error("typescript_archive_inventory_disagrees")
    return files


class Runtime:
    def __init__(self, root, contents, *, materialized=False):
        self.root = pathlib.Path(root)
        self.node = self.root / "node"
        self.compiler = self.root / "typescript/bin/tsc"
        self.inventory = []
        for name, data in sorted(contents.items()):
            if materialized:
                self.inventory.append(data)
                continue
            destination = self.root / name
            v1._write_snapshot_file(destination, data, 0o700 if name == "node" else 0o600)
            os.chmod(destination, 0o500 if name == "node" else 0o400)
            self.inventory.append({"path": name, "bytes": len(data), "sha256": p.digest(data),
                                   "mode": 0o500 if name == "node" else 0o400})
        for directory, _, _ in os.walk(self.root, topdown=False):
            os.chmod(directory, 0o500)

    def check(self):
        actual = set()
        for directory, subdirs, files in os.walk(self.root, followlinks=False):
            if pathlib.Path(directory).is_symlink() or any((pathlib.Path(directory) / x).is_symlink() for x in subdirs):
                raise p.Error("runtime_directory_substituted")
            for name in files:
                actual.add(str((pathlib.Path(directory) / name).relative_to(self.root)))
        if actual != {row["path"] for row in self.inventory}:
            raise p.Error("runtime_inventory_drifted")
        for row in self.inventory:
            path = self.root / row["path"]
            size, digest = p.file_digest(path, 128 * 1024 * 1024)
            if size != row["bytes"] or digest != row["sha256"] or (path.stat().st_mode & 0o777) != row["mode"]:
                raise p.Error("runtime_content_identity_drifted")

    def dispose(self):
        # Only our newly materialized private tree; never caller provisioned
        # archives or an existing installation. Make directories removable.
        for directory, _, _ in os.walk(self.root):
            os.chmod(directory, 0o700)


def prepare(directory, runtime_root):
    node, typescript, receipt = p.approved_archives(directory)
    files = scan_archive(node, "node", output_root=runtime_root)
    del node
    if files["node"]["sha256"] != p.NODE_BINARY_HASH:
        raise p.Error("node_selected_binary_drifted")
    fd, _ = p.open_regular(pathlib.Path(runtime_root) / "node", 128 * 1024 * 1024)
    try:
        receipt["node_libraries"] = p.check_macho(os.read(fd, 1024 * 1024))
    finally:
        os.close(fd)
    files.update(scan_archive(typescript, "typescript", output_root=runtime_root))
    del typescript
    runtime = Runtime(runtime_root, files, materialized=True)
    runtime.check()
    receipt["runtime_inventory"] = runtime.inventory
    return runtime, receipt
