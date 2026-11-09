"""Focused V11 transport controls; no source admission or runtime claims."""
import copy
import json
from pathlib import Path
import unittest
import overlay_page as codec


class DigestPins(unittest.TestCase):
    def test_complete_frozen_v10_inverse_and_unchanged_v9_chain(self):
        root = Path(__file__).parent
        raw = (root / 'v10-unqualified-full-union.json').read_bytes()
        old = codec.parse(raw)
        compact = codec.encode_v11_digest_pins(old)
        restored = codec.expand_v11_digest_pins(compact)
        self.assertEqual(codec.canonical(restored), raw)
        self.assertLess(len(codec.canonical(compact)), len(raw))
        self.assertEqual(codec.canonical(codec.expand_v10_archive_prefixes(restored)),
                         (root / 'v10-unqualified-full-union-v9.json').read_bytes())

    def test_all_byte_patterns_and_strict_digest_spelling(self):
        for seed in range(256):
            sha = bytes((seed + index) % 256 for index in range(32)).hex()
            encoded = codec.v11_digest(sha, True)
            self.assertEqual(len(encoded), 43)
            self.assertEqual(codec.v11_digest(encoded), sha)
        for bad in [True, False, None, 1, [], {}, 'A' * 42, 'A' * 44,
                    '=' * 43, '/' * 43, '+' * 43, 'A' * 42 + 'B', 'é' * 43]:
            with self.subTest(bad=bad), self.assertRaises((AssertionError, ValueError)):
                codec.v11_digest(bad)
        for bad in ['A' * 64, '0' * 63, 'g' * 64, True]:
            with self.assertRaises(AssertionError):
                codec.v11_digest(bad, True)

    def test_documented_slots_only_and_unknown_objects_remain_literal(self):
        sha = '00' * 32
        literal = dict(length=True, sha256=sha, archive='literal', extra=[sha])
        old = dict(schema=codec.V10_SCHEMA, archive_prefixes=['unchanged/'],
                   replacements=[dict(old519_index=[0, [1, sha], [['old', 2, sha, [0, 'kept']]],
                       [dict(current=[3, sha], views=[]),
                        dict(canonical_source='reference', current=[False, 'literal'])]])],
                   additions=[dict(current=literal, views=[['named', 4, sha, 'literal']], fragments=None)],
                   unknown=dict(current=[5, sha], views=[sha]))
        compact = codec.encode_v11_digest_pins(old)
        self.assertEqual(codec.expand_v11_digest_pins(compact), old)
        self.assertEqual(compact['unknown'], old['unknown'])
        self.assertEqual(compact['additions'][0]['current'], literal)
        for bad in [[], [1], [1, 'A', 0, 0], [True, 'A' * 43],
                    [-1, 'A' * 43], [codec.SOURCE_CAP + 1, 'A' * 43],
                    [1, False], [1, 'A' * 64]]:
            hostile = copy.deepcopy(compact)
            hostile['replacements'][0]['old519_index'][1] = bad
            with self.subTest(bad=bad), self.assertRaises(AssertionError):
                codec.expand_v11_digest_pins(hostile)
        for change in [lambda x: x.update(schema=codec.V10_SCHEMA),
                       lambda x: x['additions'][0].update(views=False),
                       lambda x: x['replacements'][0].update(unknown=True),
                       lambda x: x['replacements'][0].update(old519_index=[0])]:
            hostile = copy.deepcopy(compact)
            change(hostile)
            with self.assertRaises(AssertionError):
                codec.expand_v11_digest_pins(hostile)

class ClosedPriorSourcePrefixes(unittest.TestCase):
    """Exact authenticated G helpers; no source acquisition/admission substitute."""
    @classmethod
    def setUpClass(cls):
        import ast, hashlib, stat
        cls.ast, cls.hashlib, cls.stat = ast, hashlib, stat
        cls.repo = Path(__file__).resolve().parents[3]
        cls.guard = cls.repo / 'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py'
        cls.current = cls._read(cls.guard, 262144).decode()
        cls.prior = cls._read(cls.repo / 'e/f043024e201073b6b8c49e91cbdc109521c14322fa8aaa8bf339ddeabec9aa86', 262144).decode()
        assert hashlib.sha256(cls.prior.encode()).hexdigest() == 'f043024e201073b6b8c49e91cbdc109521c14322fa8aaa8bf339ddeabec9aa86'
        cls.now, cls.old = cls._pure(cls.current), cls._pure(cls.prior)

    @classmethod
    def _read(cls, path, maximum):
        identity = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
        before = path.lstat()
        assert cls.stat.S_ISREG(before.st_mode) and not path.is_symlink() and before.st_size <= maximum
        with path.open('rb') as stream:
            raw = stream.read(maximum + 1)
        assert len(raw) == before.st_size and identity(before) == identity(path.lstat())
        return raw

    @classmethod
    def _pure(cls, source):
        functions = {'_closed_source_json', '_closed_source_parts', '_decode_configuration', '_encode_configuration',
                     '_decode_configuration_v1', '_encode_configuration_v1', '_decode_configuration_v2', '_encode_configuration_v2'}
        constants = {'_CLOSED_SOURCE_SCHEMA', '_CLOSED_SOURCE_PREFIXES', '_CLOSED_SOURCE_SCHEMA_V2', '_CLOSED_SOURCE_PREFIXES_V2'}
        nodes = [n for n in cls.ast.parse(source).body
                 if isinstance(n, cls.ast.FunctionDef) and n.name in functions
                 or isinstance(n, cls.ast.Assign) and len(n.targets) == 1
                    and isinstance(n.targets[0], cls.ast.Name) and n.targets[0].id in constants]
        namespace = {'json': json}
        exec(compile(cls.ast.Module(body=nodes, type_ignores=[]), str(cls.guard), 'exec'), namespace)
        return namespace

    def _logical(self):
        prefixes = self.now['_CLOSED_SOURCE_PREFIXES_V2']
        paths = [prefix + 'owner.rs' for _, prefix in prefixes]
        return {'closed_prior_sources': paths + ['unmatched/Ω.rs', paths[0]],
                'modules': {'opaque': {'closed_prior_sources': ['R:untouched'], 'length': True}},
                'opaque': [None, 0, False, {'L:lookalike': 'src/loan_plan/untouched'}]}

    def test_complete_original_v1_helpers_table_and_legacy_behavior_remain_exact(self):
        for name in ['_closed_source_json', '_closed_source_parts', '_decode_configuration', '_encode_configuration']:
            prior = next(n for n in self.ast.parse(self.prior).body if isinstance(n, self.ast.FunctionDef) and n.name == name)
            current_name = name + '_v1' if name in ('_decode_configuration', '_encode_configuration') else name
            current = next(n for n in self.ast.parse(self.current).body if isinstance(n, self.ast.FunctionDef) and n.name == current_name)
            self.assertEqual(self.ast.get_source_segment(self.current, current).replace('def ' + current_name + '(', 'def ' + name + '(', 1),
                             self.ast.get_source_segment(self.prior, prior))
        self.assertEqual(self.now['_CLOSED_SOURCE_PREFIXES'], self.old['_CLOSED_SOURCE_PREFIXES'])
        table = self.now['_CLOSED_SOURCE_PREFIXES_V2']
        self.assertEqual(table[:8], self.old['_CLOSED_SOURCE_PREFIXES'])
        self.assertEqual(len(table), len(set(code for code, _ in table)))
        self.assertTrue(all(not a.startswith(b) and not b.startswith(a)
                            for i, (_, a) in enumerate(table) for _, b in table[i + 1:]))
        logical = self._logical()
        self.assertIs(self.now['_decode_configuration'](logical), logical)
        old_wire = self.old['_encode_configuration'](logical)
        self.assertEqual(self.now['_encode_configuration_v1'](logical), old_wire)
        self.assertEqual(self.now['_decode_configuration'](old_wire), logical)

    def test_typed_slot_only_entire_unknown_fields_order_and_duplicate_inverse(self):
        import copy
        logical = self._logical()
        before = copy.deepcopy(logical)
        wire = self.now['_encode_configuration'](logical)
        self.assertEqual(wire['closed_prior_sources']['schema'], self.now['_CLOSED_SOURCE_SCHEMA_V2'])
        self.assertEqual(list(wire), list(logical))
        self.assertEqual(wire['modules'], logical['modules'])
        self.assertEqual(wire['opaque'], logical['opaque'])
        self.assertEqual(self.now['_decode_configuration'](wire), logical)
        self.assertEqual(logical, before)
        self.assertEqual(wire['closed_prior_sources']['sources'][0], wire['closed_prior_sources']['sources'][-1])
        self.assertEqual(wire['closed_prior_sources']['sources'][-2], 'unmatched/Ω.rs')

    def test_versions_keep_distinct_canonical_paths_and_refuse_hostile_envelopes(self):
        import copy
        for schema in [self.now['_CLOSED_SOURCE_SCHEMA'], self.now['_CLOSED_SOURCE_SCHEMA_V2']]:
            base = {'closed_prior_sources': {'schema': schema, 'sources': ['R:owner.rs']}, 'unknown': [1, 2]}
            for bad in [True, 4, None, {}, [], '', 'Z:owner.rs', 'R:', 'R:a:b', 'R:/a', 'R:a//b', 'R:../a', 'R:a/./b', 'R:a\\b', 'R:a\x00b', '/absolute', 'src/cleanup_plan/replay/owner.rs']:
                wire = copy.deepcopy(base)
                wire['closed_prior_sources']['sources'] = [bad]
                with self.subTest(schema=schema, bad=bad), self.assertRaises((AssertionError, UnicodeError)):
                    self.now['_decode_configuration'](wire)
            for change in [lambda f: f.update(extra=None), lambda f: f.update(schema='unknown'), lambda f: f.update(sources=True)]:
                wire = copy.deepcopy(base)
                change(wire['closed_prior_sources'])
                with self.assertRaises(AssertionError): self.now['_decode_configuration'](wire)
        v1 = {'closed_prior_sources': {'schema': self.now['_CLOSED_SOURCE_SCHEMA'], 'sources': ['src/loan_plan/owner.rs']}}
        self.assertEqual(self.now['_decode_configuration'](v1)['closed_prior_sources'], ['src/loan_plan/owner.rs'])
        v1['closed_prior_sources']['sources'] = ['L:owner.rs']
        with self.assertRaises(AssertionError): self.now['_decode_configuration'](v1)
        v1['closed_prior_sources']['schema'] = self.now['_CLOSED_SOURCE_SCHEMA_V2']
        self.assertEqual(self.now['_decode_configuration'](v1)['closed_prior_sources'], ['src/loan_plan/owner.rs'])
        v1['closed_prior_sources']['sources'] = ['src/loan_plan/owner.rs']
        with self.assertRaises(AssertionError): self.now['_decode_configuration'](v1)

    def test_exact_physical_wire_bound_and_expansion_preflight_precede_join(self):
        for schema in [self.now['_CLOSED_SOURCE_SCHEMA'], self.now['_CLOSED_SOURCE_SCHEMA_V2']]:
            wire = {'closed_prior_sources': {'schema': schema, 'sources': ['R:a']}, 'opaque': ''}
            size = len(self.now['_closed_source_json'](wire)) + 1
            wire['opaque'] = 'x' * (262144 - size)
            self.now['_decode_configuration'](wire)
            wire['opaque'] += 'x'
            with self.assertRaises(AssertionError): self.now['_decode_configuration'](wire)
        # Instrument a trusted table in an isolated pure namespace solely to
        # expose the pre-join bound; the wire cannot supply any prefix table.
        class LengthOnly:
            def __len__(self): return 9753094
        class NoJoin(str):
            def encode(self, *args): return LengthOnly()
            def __add__(self, suffix): raise RuntimeError('joined before expanded bound')
        namespace = self._pure(self.current)
        namespace['_CLOSED_SOURCE_PREFIXES_V2'] = (('R', NoJoin('src/cleanup_plan/replay/')),)
        wire = {'closed_prior_sources': {'schema': namespace['_CLOSED_SOURCE_SCHEMA_V2'], 'sources': ['R:a']}}
        with self.assertRaises(AssertionError): namespace['_decode_configuration'](wire)

    def test_encoder_preserves_legacy_list_but_refuses_invalid_new_spelling(self):
        legacy = {'closed_prior_sources': [True, None], 'opaque': 'original legacy reader'}
        self.assertIs(self.now['_decode_configuration'](legacy), legacy)
        for path in ['src/loan_plan/', '../bad', 'R:already-tokenized', '', 'a\\b', 'a\x00b']:
            with self.subTest(path=path), self.assertRaises(AssertionError):
                self.now['_encode_configuration']({'closed_prior_sources': [path]})

    def test_actual_complete_configuration_roundtrips_all_current_fields_and_pins(self):
        cfg_path = self.repo / 'docs/evidence/merge-2e92-browser-20261018/guard-config.json'
        raw = self._read(cfg_path, 262144)
        assignments = {n.targets[0].id: self.ast.literal_eval(n.value) for n in self.ast.parse(self.current).body
                       if isinstance(n, self.ast.Assign) and len(n.targets) == 1 and isinstance(n.targets[0], self.ast.Name)
                          and n.targets[0].id in ('_CONFIG_LENGTH', '_CONFIG_SHA256')}
        self.assertEqual(len(raw), assignments['_CONFIG_LENGTH'])
        self.assertEqual(self.hashlib.sha256(raw).hexdigest(), assignments['_CONFIG_SHA256'])
        wire = json.loads(raw)
        logical = self.now['_decode_configuration'](wire)
        self.assertEqual(self.now['_decode_configuration'](self.now['_encode_configuration'](logical)), logical)
        self.assertEqual(self.now['_closed_source_json'](wire) + b'\n', raw)
        if wire['closed_prior_sources']['schema'] == self.now['_CLOSED_SOURCE_SCHEMA_V2']:
            self.assertEqual(self.now['_encode_configuration'](logical), wire)
        for key in wire:
            if key != 'closed_prior_sources': self.assertEqual(wire[key], logical[key])
        self.assertEqual(self._read(cfg_path, 262144), raw)
        self.assertEqual(self._read(self.guard, 262144).decode(), self.current)


if __name__ == '__main__':
    unittest.main()
