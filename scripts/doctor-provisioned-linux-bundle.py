#!/usr/bin/env python3
"""Assemble one real offline doctor bundle and its worker request.

`scripts/doctor-provisioned-linux-gate.py` needs `SEMAPRAX_DOCTOR_REAL_BUNDLE`:
a canonical `SPXDOC1\\0` inventory carrying real Clang, Node and Rust
distributions. Nothing in the tree writes one -- every caller of
`encode_doctor_offline_bundle` builds bytes in memory for a unit test -- so a
provisioner has to produce the carrier itself.

This script is a *packager*, not an authority. It reads exactly the absolute
host paths it is given, records their bytes under those same pathnames, and
emits the bytes the sole Rust wire validator will re-check. It authenticates no
distribution, grants nothing, and never consults `PATH`, a package manager or
the network. If it emits anything the validator rejects, the gate fails closed
on the malformed carrier exactly as it would on any other bad input.

The pivoted worker root contains the inventory and nothing else, so a dynamic
tool only runs if its whole loader closure is in the inventory at the paths its
`PT_INTERP` and `DT_NEEDED`/`DT_RUNPATH` lookups will use. `--closure` resolves
that closure with `ldd` and records each file under the exact name the loader
opens it by, which is why the lookup paths are preserved rather than rewritten
to the physical paths a usrmerge or SONAME symlink hides behind them.

    scripts/doctor-provisioned-linux-bundle.py \\
        --selector real-distributions \\
        --clang /usr/lib/llvm-18/bin/clang \\
        --node /usr/local/bin/node \\
        --rustc "$(rustup which rustc)" \\
        --closure \\
        --bundle /out/bundle.bin --request /out/request.bin

`--plan` prints the inventory, per-file sizes and the total against the 1 GiB
carrier ceiling without writing anything.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import struct
import subprocess
import sys

MAGIC = b"SPXDOC1\0"
REQUEST_MAGIC = b"SPXDWK1\0"
ABSENT = 0xFFFFFFFF

# Mirrors DOCTOR_OFFLINE_INPUT_MAX_BYTES and the wire decoder's bounds. These
# are the contract's limits, restated so this packager refuses early with a
# useful message instead of emitting bytes the validator will reject.
MAX_BYTES = 1024 * 1024 * 1024
MAX_FILES = 4096
MAX_PATH_BYTES = 1024
MAX_TOTAL_PATH_BYTES = 1024 * 1024
MAX_COMPONENTS = 32
MAX_COMPONENT_BYTES = 255

# DoctorOfflineArchitecture::LinuxX86_64. This packager admits one target for
# the same reason the gate does: issue #61 scopes exactly one environment.
ARCHITECTURE = 1
ELF_MACHINE = 62

# DoctorOfflineTarget, and the role mask each target requires.
TARGETS = {"contributor": 0, "native": 1, "web": 2, "all": 3}
TARGET_ROLES = (4, 1, 2, 7)

# Role ordinals in the header's index triple.
ROLE_NAMES = ("clang", "node", "rustc")

_COMPONENT = re.compile(rb"^[A-Za-z0-9._+-]+$")


class Rejected(Exception):
    """The requested inventory is not one the wire validator would admit."""


def validate_path(path: str) -> None:
    """The decoder's `validate_path`, restated so a bad path fails here."""
    raw = path.encode("utf-8")
    if not raw or len(raw) > MAX_PATH_BYTES:
        raise Rejected(f"path length is not 1..={MAX_PATH_BYTES}: {path!r}")
    components = raw.split(b"/")
    if len(components) > MAX_COMPONENTS:
        raise Rejected(f"path has more than {MAX_COMPONENTS} components: {path!r}")
    for component in components:
        if not component or len(component) > MAX_COMPONENT_BYTES:
            raise Rejected(f"path component is not 1..={MAX_COMPONENT_BYTES}: {path!r}")
        if component in (b".", b".."):
            raise Rejected(f"path carries a relative component: {path!r}")
        if not _COMPONENT.match(component):
            raise Rejected(f"path component is outside the admitted bytes: {path!r}")


def valid_selector(selector: str) -> bool:
    raw = selector.encode("utf-8")
    return (
        1 <= len(raw) <= 64
        and 0x61 <= raw[0] <= 0x7A
        and all(0x61 <= b <= 0x7A or 0x30 <= b <= 0x39 or b == 0x2D for b in raw)
    )


def elf_interpreter(content: bytes, path: str) -> str | None:
    """Return the `PT_INTERP` path, restating the decoder's ELF admission.

    This is a refusal check, not a second ELF implementation for any product
    route: it exists so a wrong-architecture or dynamically-loaded image is
    reported here rather than as an opaque wire rejection later.
    """
    if len(content) < 64 or content[:7] != b"\x7fELF\x02\x01\x01":
        raise Rejected(f"{path} is not a 64-bit little-endian ELF image")
    (e_type, e_machine, e_version) = struct.unpack_from("<HHI", content, 16)
    if e_type not in (2, 3):
        raise Rejected(f"{path} ELF type is {e_type}, not ET_EXEC or ET_DYN")
    if e_machine != ELF_MACHINE:
        raise Rejected(f"{path} ELF machine is {e_machine}, not {ELF_MACHINE}")
    if e_version != 1:
        raise Rejected(f"{path} ELF version is {e_version}, not 1")
    (e_phoff,) = struct.unpack_from("<Q", content, 32)
    (e_ehsize, e_phentsize, e_phnum) = struct.unpack_from("<HHH", content, 52)
    if e_ehsize != 64 or e_phentsize != 56:
        raise Rejected(f"{path} ELF header sizes are not the admitted 64/56")
    if not 1 <= e_phnum <= 128:
        raise Rejected(f"{path} has {e_phnum} program headers, not 1..=128")
    end = e_phoff + e_phentsize * e_phnum
    if e_phoff < 64 or end > len(content):
        raise Rejected(f"{path} program header table is out of bounds")
    interpreter = None
    for index in range(e_phnum):
        base = e_phoff + index * e_phentsize
        (p_type,) = struct.unpack_from("<I", content, base)
        if p_type != 3:
            continue
        if interpreter is not None:
            raise Rejected(f"{path} carries more than one PT_INTERP")
        (p_offset,) = struct.unpack_from("<Q", content, base + 8)
        (p_filesz,) = struct.unpack_from("<Q", content, base + 32)
        if not 3 <= p_filesz <= 1026 or p_offset + p_filesz > len(content):
            raise Rejected(f"{path} PT_INTERP is out of bounds")
        raw = content[p_offset : p_offset + p_filesz]
        if raw[-1] != 0 or 0 in raw[:-1]:
            raise Rejected(f"{path} PT_INTERP is not one NUL-terminated string")
        interpreter = raw[:-1].decode("utf-8")
        if not interpreter.startswith("/"):
            raise Rejected(f"{path} PT_INTERP {interpreter!r} is not absolute")
    return interpreter


def resolve_closure(binary: str) -> list[str]:
    """Every file `ld.so` opens for `binary`, at the names it opens them by.

    The root holds the inventory and nothing else, so an omitted object is a
    tool that cannot start. `ldd` is the loader's own answer; this script does
    not reimplement dynamic resolution.

    The names matter as much as the bytes. A usrmerge host reaches
    `/lib/x86_64-linux-gnu/libc.so.6` through a `/lib -> usr/lib` symlink, and
    a versioned object through its SONAME symlink, but the inventory carries no
    symlinks: the loader inside the pivoted root opens the literal path it was
    given. So each entry is recorded under the lookup path, resolved only for
    `.` and `..`, and never rewritten to the physical path behind it.
    """
    finished = subprocess.run(
        ["ldd", binary], capture_output=True, text=True, check=False
    )
    if finished.returncode != 0:
        raise Rejected(f"ldd {binary} failed: {finished.stderr.strip()}")
    resolved = []
    for line in finished.stdout.splitlines():
        for token in line.split():
            if token.startswith("/") and os.path.isfile(token):
                resolved.append(token)
    return resolved


def collect(arguments) -> dict[str, str]:
    """Map each inventory path to the host file whose bytes it carries."""
    inventory: dict[str, str] = {}

    def record(host: str) -> str:
        lookup = os.path.normpath(os.path.abspath(host))
        if not os.path.isfile(lookup):
            raise Rejected(f"{host} is not a regular file")
        path = lookup.lstrip("/")
        validate_path(path)
        previous = inventory.setdefault(path, lookup)
        if os.path.realpath(previous) != os.path.realpath(lookup):
            raise Rejected(f"{path} would carry two different files")
        return path

    roles: dict[str, str] = {}
    for name, host in (
        ("clang", arguments.clang),
        ("node", arguments.node),
        ("rustc", arguments.rustc),
    ):
        if host is None:
            continue
        path = record(host)
        if os.path.basename(path) != name:
            raise Rejected(
                f"the {name} role must be carried by a file named {name!r}, "
                f"not {os.path.basename(path)!r}"
            )
        roles[name] = path
        if arguments.closure:
            for member in resolve_closure(host):
                record(member)
    if not roles:
        raise Rejected("at least one role must be supplied")
    for host in arguments.include:
        record(host)
    return inventory, roles


def build(
    inventory: dict[str, str],
    roles: dict[str, str],
    selector: str,
    measure_only: bool = False,
) -> bytes:
    if not valid_selector(selector):
        raise Rejected(f"selector {selector!r} is not canonical")
    paths = sorted(inventory, key=lambda path: path.encode("utf-8"))
    if not paths:
        raise Rejected("the inventory is empty")
    if len(paths) > MAX_FILES:
        raise Rejected(f"{len(paths)} files exceeds the {MAX_FILES} ceiling")
    total_path_bytes = sum(len(path.encode("utf-8")) for path in paths)
    if total_path_bytes > MAX_TOTAL_PATH_BYTES:
        raise Rejected("total path bytes exceed the 1 MiB ceiling")
    ordinals = {path: index for index, path in enumerate(paths)}
    for path in paths:
        for offset, byte in enumerate(path):
            if byte == "/" and path[:offset] in ordinals:
                raise Rejected(f"{path[:offset]} is both a file and an ancestor")

    contents = []
    interpreters = {}
    for path in paths:
        with open(inventory[path], "rb") as handle:
            content = handle.read()
        executable = content[:4] == b"\x7fELF"
        if executable:
            interpreters[path] = elf_interpreter(content, path)
        contents.append((path, content, executable))
    for path, interpreter in interpreters.items():
        if interpreter is None:
            continue
        target = interpreter.lstrip("/")
        if target not in ordinals:
            raise Rejected(
                f"{path} needs the loader {interpreter} and the inventory "
                "does not carry it"
            )
        if interpreters.get(target) is not None:
            raise Rejected(f"the loader {interpreter} itself needs a loader")

    role_mask = 0
    indices = [ABSENT, ABSENT, ABSENT]
    for ordinal, name in enumerate(ROLE_NAMES):
        if name not in roles:
            continue
        role_mask |= 1 << ordinal
        indices[ordinal] = ordinals[roles[name]]

    encoded_selector = selector.encode("utf-8")
    out = bytearray(MAGIC)
    out.append(ARCHITECTURE)
    out.append(role_mask)
    out += struct.pack("<H", len(encoded_selector))
    out += struct.pack("<I", len(paths))
    for index in indices:
        out += struct.pack("<I", index)
    out += encoded_selector
    for path, content, executable in contents:
        encoded_path = path.encode("utf-8")
        out += struct.pack("<H", len(encoded_path))
        out += bytes((1 if executable else 0, 0))
        out += struct.pack("<Q", len(content))
        out += encoded_path
        out += content
    if len(out) > MAX_BYTES and not measure_only:
        raise Rejected(
            f"the encoded bundle is {len(out)} bytes, over the "
            f"{MAX_BYTES}-byte carrier ceiling; the inventory does not fit"
        )
    return bytes(out)


def request(bundle: bytes, selector: str, target: str, nonce: bytes) -> bytes:
    ordinal = TARGETS[target]
    encoded_selector = selector.encode("utf-8")
    if not any(nonce):
        raise Rejected("the request nonce must not be all zero")
    out = bytearray(REQUEST_MAGIC)
    out += bytes((1, ARCHITECTURE, ordinal, TARGET_ROLES[ordinal]))
    out += nonce
    out += struct.pack("<Q", len(bundle))
    out += hashlib.sha256(bundle).digest()
    out.append(len(encoded_selector))
    out += encoded_selector
    return bytes(out)


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--selector", required=True)
    parser.add_argument("--clang")
    parser.add_argument("--node")
    parser.add_argument("--rustc")
    parser.add_argument(
        "--closure",
        action="store_true",
        help="carry each role's whole ldd closure, including its PT_INTERP",
    )
    parser.add_argument(
        "--include",
        action="append",
        default=[],
        help="carry one more host file under its own resolved path",
    )
    parser.add_argument("--bundle")
    parser.add_argument("--request")
    parser.add_argument("--target", default="all", choices=sorted(TARGETS))
    parser.add_argument(
        "--nonce",
        default="37" * 32,
        help="64 hexadecimal digits binding this invocation's bytes",
    )
    parser.add_argument("--plan", action="store_true")
    arguments = parser.parse_args(argv)

    inventory, roles = collect(arguments)
    # `--plan` measures; it never writes. An inventory that overruns the carrier
    # ceiling is exactly the answer a provisioner is asking for, so measuring
    # reports the overrun instead of refusing before it can print the numbers.
    # Emission still refuses: only `--plan` tolerates an oversized inventory.
    encoded = build(inventory, roles, arguments.selector, arguments.plan)
    if arguments.plan:
        for path in sorted(inventory, key=lambda path: path.encode("utf-8")):
            print(f"{os.path.getsize(inventory[path]):>12} /{path}")
        over = len(encoded) - MAX_BYTES
        verdict = f"OVER BY {over}" if over > 0 else f"{-over} to spare"
        print(f"{len(encoded):>12} TOTAL (ceiling {MAX_BYTES}: {verdict})")
        for name, path in sorted(roles.items()):
            print(f"role {name}: /{path}")
        return 1 if over > 0 else 0
    if not arguments.bundle:
        parser.error("--bundle is required unless --plan is given")
    with open(arguments.bundle, "wb") as handle:
        handle.write(encoded)
    if arguments.request:
        nonce = bytes.fromhex(arguments.nonce)
        if len(nonce) != 32:
            raise Rejected("the nonce must be 32 bytes of hexadecimal")
        with open(arguments.request, "wb") as handle:
            handle.write(request(encoded, arguments.selector, arguments.target, nonce))
    print(f"{arguments.bundle}: {len(encoded)} bytes, {len(inventory)} files")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Rejected as rejected:
        print(f"error: {rejected}", file=sys.stderr)
        print(
            "This is a failure, not a skip. A carrier the validator would "
            "reject is never a passing confinement result.",
            file=sys.stderr,
        )
        sys.exit(1)
