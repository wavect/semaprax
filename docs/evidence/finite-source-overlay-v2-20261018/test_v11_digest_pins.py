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


if __name__ == '__main__':
    unittest.main()
