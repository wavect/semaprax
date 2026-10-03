"""Bounded archive admission tests; no provider calls or credentials."""
import io
import pathlib
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2]))
from agent import pilot_linux_runtime as runtime


def archive(path, rows):
    with tarfile.open(path, 'w') as output:
        for name, value, kind in rows:
            entry = tarfile.TarInfo(name)
            if kind == 'file':
                entry.size = len(value)
                entry.mode = 0o644
                output.addfile(entry, io.BytesIO(value))
            else:
                entry.type = tarfile.SYMTYPE
                entry.linkname = value
                output.addfile(entry)


class LinuxRuntimeTests(unittest.TestCase):
    def test_source_preserves_bytes_and_has_no_git_authority(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            source = root / 'source.tar'
            archive(source, [('agent/module.py', b'print("fixture")\n', 'file')])
            inventory = runtime.materialize_source(source, root / 'projection')
            self.assertEqual(inventory[0]['sha256'], runtime.p.digest(b'print("fixture")\n'))
            self.assertEqual((root / 'projection/agent/module.py').stat().st_mode & 0o777, 0o400)
            self.assertFalse((root / 'projection/.git').exists())
            with self.assertRaises(FileExistsError):
                runtime.materialize_source(source, root / 'projection')

    def test_source_refuses_paths_links_and_case_collisions(self):
        rows = [('../escape', b'x', 'file'), ('.git/config', b'x', 'file'),
                ('/absolute', b'x', 'file'), ('link', '/etc/passwd', 'link')]
        for row in rows:
            with self.subTest(row=row), tempfile.TemporaryDirectory() as temporary:
                root = pathlib.Path(temporary).resolve()
                source = root / 'source.tar'
                archive(source, [row])
                with self.assertRaises(ValueError):
                    runtime.materialize_source(source, root / 'projection')
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            source = root / 'source.tar'
            archive(source, [('A', b'a', 'file'), ('a', b'b', 'file')])
            with self.assertRaises(ValueError):
                runtime.materialize_source(source, root / 'projection')

    def test_python_digest_precedes_extraction_and_guest_exec(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary).resolve()
            source = root / 'bad.tar.gz'
            source.write_bytes(b'not the pinned archive')
            with mock.patch.object(runtime, 'checked_root', return_value=(root, {}, '/container', {})), \
                    mock.patch.object(runtime, 'materialize_python') as extract, \
                    mock.patch.object(runtime.subprocess, 'run') as run:
                with self.assertRaisesRegex(ValueError, 'python_archive_identity_refused'):
                    runtime.stage_python(source, root, '0' * 64)
                extract.assert_not_called()
                run.assert_not_called()

    def test_python_selection_excludes_unused_case_colliding_terminfo(self):
        self.assertTrue(runtime.selected('python/bin/python3.12'))
        self.assertTrue(runtime.selected('python/lib/python3.12/json/__init__.py'))
        self.assertFalse(runtime.selected('python/share/terminfo/a/aixterm'))
        self.assertFalse(runtime.selected('python/bin/pip'))


if __name__ == '__main__':
    unittest.main()
