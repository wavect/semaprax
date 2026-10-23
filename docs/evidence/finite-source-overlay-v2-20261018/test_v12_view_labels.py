"""Lossless V12 transport controls; no source admission or runtime claims."""
import copy
import hashlib
from pathlib import Path
import runpy
import unittest
import overlay_page as codec

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / 'src/source_reflows_merge/main_e177_overlay_labels_v12'
OLD = runpy.run_path(str(FIXTURE / 'controller.before.py.txt'))


class ViewLabels(unittest.TestCase):
    def pair(self):
        return tuple(codec.parse((FIXTURE / name).read_bytes())
                     for name in ('parity-v11.json', 'parity-v12.json'))

    def test_shared_exact_vector_preserves_repeated_views_unknown_metadata_and_default(self):
        full, compact = self.pair()
        self.assertEqual(codec.canonical(codec.encode_v12_view_labels(full)),
                         (FIXTURE / 'parity-v12.json').read_bytes())
        self.assertEqual(codec.expand_v12_view_labels(compact), full)
        self.assertEqual(compact['view_labels'], [[0, 'same'], [1, 'other']])
        self.assertEqual(compact['replacements'][0]['old519_index'][2][0][0], 0)
        self.assertEqual(compact['replacements'][0]['old519_index'][2][1][0], 0)
        literal = full['replacements'][0]['old519_index'][2][2]
        self.assertEqual(compact['replacements'][0]['old519_index'][2][2], literal)
        frozen = codec.parse((Path(__file__).parent / 'v10-unqualified-full-union.json').read_bytes())
        self.assertEqual(codec.canonical(codec.encode_v11_digest_pins(frozen)),
                         OLD['canonical'](OLD['encode_v11_digest_pins'](frozen)))

    def test_complete_frozen_v11_object_and_unchanged_v9_chain(self):
        raw = (FIXTURE / 'v11.before.json').read_bytes()
        full = codec.parse(raw)
        compact = codec.encode_v12_view_labels(full)
        restored = codec.expand_v12_view_labels(compact)
        self.assertEqual(codec.canonical(restored), raw)
        self.assertEqual(compact['archive_prefixes'], full['archive_prefixes'])
        self.assertEqual(len(full['archive_prefixes']), 23)
        old_v9 = OLD['expand_v10_archive_prefixes'](OLD['expand_v11_digest_pins'](full))
        current_v9 = codec.expand_v10_archive_prefixes(codec.expand_v11_digest_pins(restored))
        self.assertEqual(codec.canonical(current_v9), OLD['canonical'](old_v9))
        self.assertLess(len(codec.canonical(compact)), len(raw))
        self.assertLessEqual(len(codec.canonical(compact)), codec.PAGE_CAP)

    def test_declaration_indices_names_usage_and_first_use_are_closed(self):
        _, compact = self.pair()
        changes = [
            lambda p: p['view_labels'].__setitem__(1, [0, 'other']),
            lambda p: p['view_labels'].__setitem__(1, [1, 'same']),
            lambda p: p['view_labels'].__setitem__(0, [True, 'same']),
            lambda p: p['view_labels'].__setitem__(0, {'index': 0, 'name': 'same', 'extra': True}),
            lambda p: p['view_labels'].append([2, 'unused']),
            lambda p: p['replacements'][0]['old519_index'][2][0].__setitem__(0, 1),
        ]
        for change in changes:
            hostile = copy.deepcopy(compact); change(hostile)
            with self.subTest(change=change), self.assertRaises(AssertionError):
                codec.expand_v12_view_labels(hostile)
        for index in [True, False, -1, 2, 2 ** 100, 'unknown', None, {}, []]:
            hostile = copy.deepcopy(compact)
            hostile['replacements'][0]['old519_index'][2][0][0] = index
            with self.subTest(index=index), self.assertRaises(AssertionError):
                codec.expand_v12_view_labels(hostile)

    def test_new_envelope_and_fixed_slot_shapes_refuse_unknown_fields(self):
        _, compact = self.pair()
        changes = [lambda p: p.update(unknown=True),
                   lambda p: p.update(schema=codec.V11_SCHEMA),
                   lambda p: p.pop('view_labels'),
                   lambda p: p.update(view_labels=False),
                   lambda p: p['replacements'][0].update(unknown=True),
                   lambda p: p['replacements'][0].update(old519_index=[0]),
                   lambda p: p['additions'][0].update(views=False),
                   lambda p: p['additions'][0].update(views=[[0]])]
        for change in changes:
            hostile = copy.deepcopy(compact); change(hostile)
            with self.subTest(change=change), self.assertRaises(AssertionError):
                codec.expand_v12_view_labels(hostile)
        raw = codec.canonical(compact).replace(b'"view_labels":', b'"view_labels":[],"view_labels":', 1)
        with self.assertRaises(AssertionError):
            codec.parse(raw)

    def test_expansion_bound_and_utf8_quoted_names_are_exact(self):
        full, compact = self.pair()
        ref = compact['replacements'][0]['old519_index']
        compact['view_labels'] = [[0, 'x' * 131072]]
        ref[2] = [[0, 1, 'A' * 43] for _ in range(100)]
        ref[3] = None; compact['additions'] = []
        self.assertLess(len(codec.canonical(compact)), codec.PAGE_CAP)
        with self.assertRaisesRegex(AssertionError, 'expanded label bound'):
            codec.expand_v12_view_labels(compact)
        name = 'é\0"\\💚'
        full['replacements'] = []
        full['additions'][0]['views'] = [[name, 0, 'A' * 43], [name, 0, 'A' * 43]]
        self.assertEqual(codec.expand_v12_view_labels(codec.encode_v12_view_labels(full)), full)

    def test_entire_previous_controller_and_caps_restore_exactly(self):
        before = (FIXTURE / 'controller.before.py.txt').read_bytes()
        self.assertEqual(hashlib.sha256(before).hexdigest(),
                         '5834ed5afd8ddaf3be4229b2a3a97edf2420b92b46ee195e44d48ea8844c71ec')
        current = (Path(__file__).parent / 'overlay_page.py').read_text()
        start = current.index("V12_SCHEMA = 'semaprax.finite-source-overlay-page.v12'\n")
        end = current.index('def compose_values(before, policy, page):\n', start)
        restored = (current[:start] + current[end:]).replace(
            " if page['schema']==V12_SCHEMA:page=expand_v12_view_labels(page)\n", '', 1)
        self.assertEqual(restored.encode(), before)
        self.assertEqual(codec.PAGE_CAP, OLD['PAGE_CAP'])
        self.assertEqual(codec.SOURCE_CAP, OLD['SOURCE_CAP'])


if __name__ == '__main__':
    unittest.main()
