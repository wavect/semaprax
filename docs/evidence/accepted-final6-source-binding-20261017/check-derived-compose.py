"""Focused #923 source controls; never execute the complete FINAL/75 checker."""
from pathlib import Path
import ast
import copy
import hashlib
import json
import os
import resource
import sys
import tempfile
import time
import unittest

D = Path(__file__).resolve().parent
R = D.parents[2]
helper = (D / 'derived_compose.py').read_bytes()
ns = {'__file__': str(D / 'derived_compose.py')}
exec(compile(helper, ns['__file__'], 'exec'), ns)
DerivedCompose, install = ns['DerivedCompose'], ns['install']
ORIGINAL_SHA = '63e0399384c8a6ccdc56b138504e4989a59cb274c0976169c69592f2e0e4917f'


def original():
    raw = (D / 'before/check-source.original.py').read_bytes()
    assert len(raw) == 3542 and hashlib.sha256(raw).hexdigest() == ORIGINAL_SHA
    return raw


def reader():
    # Compile the complete original physical reader, with no new read semantics.
    tree = ast.parse(original())
    function = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                    and node.name == 'bounded')
    module = ast.Module(body=[function], type_ignores=[])
    environment = dict(os=os, stat=__import__('stat'), hashlib=hashlib)
    exec(compile(module, '<exact-original-bounded>', 'exec'), environment)
    return environment['bounded']


class Controls(unittest.TestCase):
    def test_compile_reuse_preserves_source_filename_mode_and_fresh_exec(self):
        calls = []
        def original(*args, **kwargs):
            calls.append((args, kwargs))
            return compile(*args, **kwargs)
        cached = ns['DerivedCompile'](original)
        source = 'items = []\nitems.append(seed)\n'
        code = cached(source, 'same-source.py', 'exec')
        first, second = {'seed': 1}, {'seed': 2}
        exec(code, first)
        exec(cached(source, 'same-source.py', 'exec'), second)
        self.assertEqual((first['items'], second['items']), ([1], [2]))
        self.assertIsNot(first['items'], second['items'])
        self.assertEqual(len(calls), 1)
        cached(source, 'other-source.py', 'exec')
        cached(source.encode(), 'same-source.py', 'exec')
        self.assertEqual(len(calls), 3)
        cached(source, 'same-source.py', 'exec')  # bounded eviction
        self.assertEqual(len(calls), 4)
        self.assertEqual(len(cached.entries), 2)
        for _ in range(2):
            with self.assertRaises(SyntaxError):
                cached('if :', 'bad.py', 'exec')
        self.assertEqual(len(calls), 6)
        for _ in range(2):
            cached('1 + 2', 'eval.py', 'eval')
            cached(source, 'same-source.py', 'exec', optimize=1)
            cached('\n' * 65537, 'large.py', 'exec')
        self.assertEqual(len(calls), 12)

    def test_compile_hit_still_executes_original_late_read_and_refuses_drift(self):
        bounded = reader()
        with tempfile.TemporaryDirectory(prefix='opt923-compile-', dir='/Volumes/Semaprax/runtime-tmp') as tmp:
            path = Path(tmp) / 'source'
            raw = b'complete physical source'
            path.write_bytes(raw)
            digest = hashlib.sha256(raw).hexdigest()
            calls, reads = [], []
            def original(*args, **kwargs):
                calls.append((args, kwargs))
                return compile(*args, **kwargs)
            cached = ns['DerivedCompile'](original)
            source = "reads.append('late')\nheld = bounded(path, length, digest)\n"
            def execute():
                namespace = dict(reads=reads, bounded=bounded, path=path,
                                 length=len(raw), digest=digest)
                exec(cached(source, 'live-read.py', 'exec'), namespace)
                return namespace['held']
            self.assertEqual(execute(), raw)
            self.assertEqual(execute(), raw)
            self.assertEqual((len(calls), reads), (1, ['late', 'late']))
            for changed in [b'X' + raw[1:], raw + b' ', b'']:
                path.write_bytes(changed)
                with self.assertRaises(AssertionError):
                    execute()
                path.write_bytes(raw)
            regular = Path(tmp) / 'regular'
            path.rename(regular)
            path.symlink_to(regular)
            with self.assertRaises(OSError):
                execute()
            self.assertEqual(len(calls), 1)
            self.assertEqual(len(reads), 6)

    def test_complete_original_and_single_insertion_transport(self):
        raw = original()
        wrapper = (D / 'check-source.py').read_text()
        tree = ast.parse(wrapper)
        bindings = {}
        for node in tree.body:
            if isinstance(node, ast.Assign) and len(node.targets) == 1:
                target = node.targets[0]
                if isinstance(target, ast.Name) and target.id == 'anchor':
                    bindings['anchor'] = ast.literal_eval(node.value)
        pin_bindings = {
            node.targets[0].id: ast.literal_eval(node.value)
            for node in tree.body if isinstance(node, ast.Assign)
            and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)
            and node.targets[0].id in ('old_binding', 'current_binding')
        }
        guard = (R / 'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py').read_bytes()
        self.assertEqual(pin_bindings['current_binding'],
                         f"helper_raw=bounded(helper,{len(guard)},'{hashlib.sha256(guard).hexdigest()}')")
        self.assertEqual(raw.decode().count(pin_bindings['old_binding']), 1)
        bound = raw.decode().replace(pin_bindings['old_binding'], pin_bindings['current_binding'], 1)
        self.assertEqual(bound.replace(pin_bindings['current_binding'], pin_bindings['old_binding'], 1).encode(), raw)
        anchor = bindings['anchor']
        self.assertEqual(raw.decode().count(anchor), 1)
        replacement = anchor + ';_install_derived(ns)'
        transported = raw.decode().replace(anchor, replacement, 1)
        self.assertEqual(transported.replace(replacement, anchor, 1).encode(), raw)
        expected = ast.get_source_segment(original().decode(), next(
            node for node in ast.parse(raw).body
            if isinstance(node, ast.FunctionDef) and node.name == 'bounded'))
        actual = ast.get_source_segment(wrapper, next(
            node for node in tree.body
            if isinstance(node, ast.FunctionDef) and node.name == 'bounded'))
        self.assertEqual(actual, expected)
        calls = [node for node in ast.walk(tree) if isinstance(node, ast.Call)
                 and isinstance(node.func, ast.Name) and node.func.id == 'bounded']
        pins = [(ast.literal_eval(node.args[1]), ast.literal_eval(node.args[2]))
                for node in calls]
        self.assertIn((len(helper), hashlib.sha256(helper).hexdigest()), pins)

    def test_success_only_one_entry_and_independent_input_output(self):
        calls = []
        def pure(before, policy, page):
            calls.append(copy.deepcopy((before, policy, page)))
            assert not page.get('bad')
            return {'before': before, 'policy': policy, 'page': page}
        cache = DerivedCompose(pure)
        before = {'unknown': {'preserved': [1, True, None]}}
        expected = cache(before, {}, {})
        before['unknown']['preserved'].append('mutated input')
        expected['before']['unknown']['preserved'].append('mutated output')
        fresh = {'unknown': {'preserved': [1, True, None]}}
        self.assertEqual(cache(fresh, {}, {}), {'before': fresh, 'policy': {}, 'page': {}})
        self.assertEqual(len(calls), 1)
        for _ in range(2):
            with self.assertRaises(AssertionError):
                cache(fresh, {}, {'bad': True})
        self.assertEqual(len(calls), 3)
        cache(fresh, {}, {})
        self.assertEqual(len(calls), 3)
        cache({'other': 1}, {}, {})
        cache(fresh, {}, {})
        self.assertEqual(len(calls), 5)
        self.assertEqual(cache.key, tuple(ns['encoded'](v) for v in (fresh, {}, {})))

    def test_exact_types_order_unknown_fields_and_oversize_bypass(self):
        calls = []
        cache = DerivedCompose(lambda *values: calls.append(values) or list(values))
        variants = [True, 1, 1.0, None, '1', [1], {'x': 1}, {'x': 1, 'y': 2},
                    {'y': 2, 'x': 1}, {'unknown': {'full': 'retained'}}]
        for value in variants:
            self.assertEqual(cache(value, {}, {}), [value, {}, {}])
        self.assertEqual(len(calls), len(variants))
        # Smaller test retention limit exercises fallback; it never raises a cap.
        old_limit = ns['SOURCE_CAP']
        ns['SOURCE_CAP'] = 32
        try:
            for _ in range(2):
                self.assertEqual(cache('x' * 33, {}, {}), ['x' * 33, {}, {}])
            self.assertEqual(len(calls), len(variants) + 2)
            output_calls = []
            large = DerivedCompose(lambda *args: output_calls.append(args) or 'x' * 33)
            large({}, {}, {}); large({}, {}, {})
            self.assertEqual(len(output_calls), 2)
            self.assertIsNone(large.key)
        finally:
            ns['SOURCE_CAP'] = old_limit
        for _ in range(2):
            cache((1,), {}, {})  # unsupported JSON type is executed, never cached
        self.assertEqual(len(calls), len(variants) + 4)

    def test_actual_complete_packet_matches_uncached_with_independent_returns(self):
        guard = guard_namespace()
        before = guard['packet']()
        install(guard)
        returned = guard['packet']()
        self.assertEqual(returned, before)
        returned['rows'][0]['unknown_foreign_field'] = {'entire': ['mutation']}
        self.assertEqual(guard['packet'](), before)

    def test_fresh_physical_authentication_precedes_every_cache_hit(self):
        bounded = reader()
        with tempfile.TemporaryDirectory(prefix='opt923-', dir='/Volumes/Semaprax/runtime-tmp') as tmp:
            path = Path(tmp) / 'source'
            data = b'{"value":1}'
            path.write_bytes(data)
            digest = hashlib.sha256(data).hexdigest()
            pure_calls, reads = [], []
            def pure(*values):
                pure_calls.append(values)
                return list(values)
            namespace = {'SOURCE_CAP': 9753094, 'compose_values': pure}
            class Owner:
                def compose(self):
                    reads.append('load')
                    raw = bounded(path, len(data), digest)
                    return namespace['compose_values'](json.loads(raw), {}, {})
            def overlay():
                reads.append('program')
                bounded(path, len(data), digest)
                return Owner()
            guard = {'_overlay': overlay, '_OVERLAY_NS': namespace}
            install(guard)
            for _ in range(2):
                self.assertEqual(guard['_overlay']().compose(), [{'value': 1}, {}, {}])
            self.assertEqual(len(pure_calls), 1)
            self.assertEqual(reads, ['program', 'load', 'program', 'load'])
            for changed in [b'{"value":2}', data + b' ', b'']:
                path.write_bytes(changed)
                with self.assertRaises(AssertionError):
                    guard['_overlay']().compose()
                path.write_bytes(data)
            regular = Path(tmp) / 'regular'
            path.rename(regular)
            path.symlink_to(regular)
            with self.assertRaises(OSError):
                guard['_overlay']().compose()
            path.unlink(); path.mkdir()
            with self.assertRaises((OSError, AssertionError)):
                guard['_overlay']().compose()
            self.assertEqual(len(pure_calls), 1)


def guard_namespace():
    path = R / 'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py'
    raw = path.read_bytes()
    assert len(raw) == 19144 and hashlib.sha256(raw).hexdigest() == 'af70eaeeb1cc0da02460858f9e7ddb39086582add858c74138c4d989644e11f3'
    guard = {'__file__': str(path), '_AUTHENTICATED_PROGRAM': raw}
    exec(compile(raw, str(path), 'exec'), guard)
    return guard


def profile(mode):
    guard = guard_namespace()
    if mode == 'memoized':
        install(guard)
    start = time.monotonic()
    outputs = [guard['packet']() for _ in range(3)]
    assert all(value == outputs[0] for value in outputs) and len(outputs[0]['rows']) == 875
    encoded = json.dumps(outputs[0], sort_keys=True, separators=(',', ':')).encode()
    print(json.dumps({'mode': mode, 'packet_calls': 3, 'rows': 875,
                      'output_sha256': hashlib.sha256(encoded).hexdigest(),
                      'wall_seconds': time.monotonic() - start,
                      'maxrss_platform_units': resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
                      'full_75_executed': False}))


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--profile':
        assert sys.argv[2] in ('original', 'memoized')
        profile(sys.argv[2])
    else:
        unittest.main()
