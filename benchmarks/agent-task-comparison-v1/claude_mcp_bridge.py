"""One-use byte bridge to a host-started, independently confined MCP process."""
from __future__ import annotations
import os
from pathlib import Path
import select
import stat
import sys


def main(arguments):
    if len(arguments) != 3:
        raise ValueError('bridge_arguments')
    incoming, outgoing, claim = map(Path, arguments)
    if any(not p.is_absolute() or p != p.resolve() for p in (incoming, outgoing, claim)):
        raise ValueError('canonical_bridge_paths')
    if len({p.parent for p in (incoming, outgoing, claim)}) != 1:
        raise ValueError('bridge_directory')
    claim_fd = os.open(claim, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    os.close(claim_fd)
    reader = os.open(incoming, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
    writer = os.open(outgoing, os.O_WRONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
    try:
        for fd in (reader, writer):
            facts = os.fstat(fd)
            if not stat.S_ISFIFO(facts.st_mode) or facts.st_uid != os.getuid() or facts.st_mode & 0o077:
                raise ValueError('private_fifo_required')
        os.set_blocking(0, False); os.set_blocking(1, False)
        to_server = bytearray(); to_cli = bytearray()
        while True:
            reads = ([0] if len(to_server) < 65536 else []) + ([reader] if len(to_cli) < 65536 else [])
            writes = ([writer] if to_server else []) + ([1] if to_cli else [])
            ready, writable, _ = select.select(reads, writes, [], 1)
            for fd in ready:
                data = os.read(fd, 16384)
                if not data:
                    return
                (to_server if fd == 0 else to_cli).extend(data)
            for fd in writable:
                queue = to_cli if fd == 1 else to_server
                try:
                    written = os.write(fd, queue)
                    del queue[:written]
                except BlockingIOError:
                    pass
    finally:
        os.close(reader); os.close(writer)


if __name__ == '__main__':
    try:
        main(sys.argv[1:])
    except (OSError, ValueError):
        sys.exit(2)
