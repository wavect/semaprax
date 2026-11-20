def read_fixed_source(path, length):
    """Read one authenticated-input candidate through a held regular file."""
    import os
    import stat

    if os.name == "nt":
        return _read_fixed_source_windows(path, length)

    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        assert stat.S_ISREG(before.st_mode) and before.st_size == length, "codec regular fixed length"
        with os.fdopen(fd, "rb", closefd=False) as stream:
            data = stream.read(length + 1)
        after = os.fstat(fd)
        named = os.lstat(path)
        assert stat.S_ISREG(after.st_mode) and stat.S_ISREG(named.st_mode), "codec postread regularity"

        def identity(info):
            return (
                info.st_dev, info.st_ino, info.st_mode, info.st_size,
                info.st_mtime_ns, info.st_ctime_ns,
            )

        assert identity(before) == identity(after) == identity(named), "codec postread identity"
        return data
    finally:
        os.close(fd)


def _read_fixed_source_windows(path, length):
    import ctypes
    from ctypes import wintypes
    import msvcrt
    import os

    class FileInfo(ctypes.Structure):
        _fields_ = [
            ("attributes", wintypes.DWORD),
            ("creation", wintypes.FILETIME),
            ("access", wintypes.FILETIME),
            ("write", wintypes.FILETIME),
            ("volume", wintypes.DWORD),
            ("size_high", wintypes.DWORD),
            ("size_low", wintypes.DWORD),
            ("links", wintypes.DWORD),
            ("index_high", wintypes.DWORD),
            ("index_low", wintypes.DWORD),
        ]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    open_file = kernel.CreateFileW
    open_file.argtypes = [
        wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, wintypes.LPVOID,
        wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE,
    ]
    open_file.restype = wintypes.HANDLE
    close_handle = kernel.CloseHandle
    close_handle.argtypes = [wintypes.HANDLE]
    close_handle.restype = wintypes.BOOL
    get_type = kernel.GetFileType
    get_type.argtypes = [wintypes.HANDLE]
    get_type.restype = wintypes.DWORD
    get_info = kernel.GetFileInformationByHandle
    get_info.argtypes = [wintypes.HANDLE, ctypes.POINTER(FileInfo)]
    get_info.restype = wintypes.BOOL

    def acquire():
        # Hold a read-only handle, reject writes and deletion while it is open,
        # and inspect the final component without following reparse points.
        handle = open_file(os.fspath(path), 0x80000000, 1, None, 3, 0x00200000, None)
        if handle == wintypes.HANDLE(-1).value:
            raise ctypes.WinError(ctypes.get_last_error())
        return handle

    def identify(handle):
        metadata = FileInfo()
        assert get_type(handle) == 1, "codec regular fixed length"
        if not get_info(handle, ctypes.byref(metadata)):
            raise ctypes.WinError(ctypes.get_last_error())
        size = (metadata.size_high << 32) | metadata.size_low
        assert not metadata.attributes & 0x410 and size == length, "codec regular fixed length"
        return (
            metadata.volume, metadata.index_high, metadata.index_low, size,
            metadata.attributes, metadata.write.dwHighDateTime, metadata.write.dwLowDateTime,
        )

    handle = acquire()
    try:
        before = identify(handle)
        fd = msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY)
    except BaseException:
        close_handle(handle)
        raise
    try:
        with os.fdopen(fd, "rb", closefd=False) as stream:
            data = stream.read(length + 1)
        after = identify(handle)
        named = acquire()
        try:
            assert before == after == identify(named), "codec postread identity"
        finally:
            close_handle(named)
        return data
    finally:
        os.close(fd)

if __name__ == "__main__":
    import hashlib
    import tempfile
    from pathlib import Path

    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        expected = b"fixed codec witness\n"
        target = root / "codec.py"
        target.write_bytes(expected)
        alias = root / "codec-link.py"
        alias.symlink_to(target)
        def load(path, length):
            result = read_fixed_source(path, length)
            if hashlib.sha256(result).digest() != hashlib.sha256(expected).digest():
                raise ValueError("unexpected codec bytes")
            return result
        assert load(target, len(expected)) == expected
        for path, length in [(root, len(expected)), (alias, len(expected)), (target, 1)]:
            try:
                load(path, length)
            except (AssertionError, OSError):
                pass
            else:
                raise RuntimeError(f"unexpected accepted input: {path}")
        target.write_bytes(b"x" + expected[1:])
        try:
            load(target, len(expected))
        except ValueError:
            pass
        else:
            raise RuntimeError("accepted incorrect bytes")
    print("Exact fixed codec transport passed")
