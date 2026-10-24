"""Closed V14 transport controls; no source admission or runtime qualification."""
import copy
import unittest
import overlay_page as codec


def fixture():
    a = 'A' * 43
    b = 'B' * 42 + 'A'
    return {'schema': codec.V12_SCHEMA, 'index': 1, 'total_pages': 1,
            'predecessor': 'kept', 'archive_prefixes': ['e/'],
            'view_labels': [[0, 'same']],
            'replacements': [{'old519_index': [0, [7, a],
                [[0, 7, a], [0, 8, b, [0, 'archive']]],
                [{'current': [8, b, [0, 'archive']], 'views': [],
                  'unknown': {'current': [7, a], 'pin_tails': [False]}},
                 {'canonical_source': 'opaque', 'current': [False, None]}]]}],
            'additions': [{'source': 'literal.rs', 'compiled': True,
                'current': {'length': 7, 'sha256': a, 'unknown': None},
                'views': [], 'extra': {'current': [7, a]}, 'fragments': None}]}


class V14PinTails(unittest.TestCase):
    def setUp(self):
        self.logical = fixture()
        self.v13 = codec._encode_v13_logical_digest_table(self.logical)
        self.compact = codec.encode_v14(self.logical)

    def reject(self, value):
        with self.assertRaises(AssertionError):
            codec.expand_v14_pin_tails(value)

    def test_entire_v13_and_v12_inverse_and_opaque_metadata(self):
        self.assertEqual(codec.expand_v14_pin_tails(self.compact), self.v13)
        self.assertEqual(codec._expand_v13_logical_digest_table(
            codec.expand_v14_pin_tails(self.compact)), self.logical)
        self.assertEqual(self.compact['additions'], self.v13['additions'])
        self.assertEqual(self.compact['replacements'][0]['old519_index'][3][1],
                         self.v13['replacements'][0]['old519_index'][3][1])
        self.assertEqual(len(self.compact['pin_tails']), 2)

    def test_reference_types_range_and_arity(self):
        for value in (True, False, -1, 2, 1.5, None, '0', [], {}):
            bad = copy.deepcopy(self.compact)
            bad['replacements'][0]['old519_index'][1] = [value]
            self.reject(bad)
        for value in ([], [0, 0, 0, 0]):
            bad = copy.deepcopy(self.compact)
            bad['replacements'][0]['old519_index'][1] = value
            self.reject(bad)

    def test_duplicate_unused_and_noncanonical_first_use(self):
        bad = copy.deepcopy(self.compact)
        bad['pin_tails'].append(copy.deepcopy(bad['pin_tails'][0]))
        self.reject(bad)
        bad = copy.deepcopy(self.compact)
        bad['pin_tails'].append([999, 0])
        self.reject(bad)
        bad = copy.deepcopy(self.compact)
        bad['replacements'][0]['old519_index'][1] = [1]
        self.reject(bad)

    def test_literal_alias_and_repeated_literal_refuse(self):
        bad = copy.deepcopy(self.compact)
        bad['replacements'][0]['old519_index'][1] = copy.deepcopy(bad['pin_tails'][0])
        self.reject(bad)
        bad = copy.deepcopy(self.compact)
        bad['pin_tails'] = []
        bad['replacements'][0]['old519_index'][1] = [7, 0]
        bad['replacements'][0]['old519_index'][2][0] = [0, 7, 0]
        self.reject(bad)

    def test_tail_declarations_and_complete_v13_validation(self):
        for value in (None, True, {}, [], [True, 0], [-1, 0],
                      [codec.SOURCE_CAP + 1, 0], [1, True], [1], [1, 0, 'e/a', 4]):
            bad = copy.deepcopy(self.compact)
            bad['pin_tails'][0] = value
            self.reject(bad)
        for value in (True, -1, 99999, 'not-a-digest'):
            bad = copy.deepcopy(self.compact)
            bad['pin_tails'][0][1] = value
            self.reject(bad)
        bad = copy.deepcopy(self.compact)
        bad['digest_table'][0] = 'A' * 42 + 'B'
        self.reject(bad)

    def test_closed_envelope_and_wire_source_caps(self):
        bad = copy.deepcopy(self.compact)
        bad['unknown'] = None
        self.reject(bad)
        bad = copy.deepcopy(self.compact)
        del bad['pin_tails']
        self.reject(bad)
        bad = copy.deepcopy(self.compact)
        bad['predecessor'] = 'x' * codec.PAGE_CAP
        self.reject(bad)
        logical = copy.deepcopy(self.logical)
        logical['additions'][0]['extra']['payload'] = 'x' * codec.SOURCE_CAP
        with self.assertRaises(AssertionError):
            codec.encode_v14(logical)

    def test_amplification_is_bounded_before_publication(self):
        bad = copy.deepcopy(self.compact)
        bad['pin_tails'] = [[7, 0, 'e/' + 'x' * 100000]]
        ref = bad['replacements'][0]['old519_index']
        ref[1] = [0]
        ref[2] = [[0, 0] for _ in range(100)]
        ref[3] = None
        self.assertLessEqual(len(codec.canonical(bad)), codec.PAGE_CAP)
        original = codec.v14_pin_slots
        calls = []
        def count_pass(page, convert):
            calls.append(True)
            return original(page, convert)
        codec.v14_pin_slots = count_pass
        try:
            self.reject(bad)
        finally:
            codec.v14_pin_slots = original
        self.assertEqual(len(calls), 1)  # validation refused before publication pass

    def test_public_v13_cap_stays_independent(self):
        logical = copy.deepcopy(self.logical)
        logical['additions'][0]['extra']['payload'] = 'x' * codec.PAGE_CAP
        with self.assertRaises(AssertionError):
            codec.encode_v13_digest_table(logical)
        v13 = codec._encode_v13_logical_digest_table(logical)
        with self.assertRaises(AssertionError):
            codec.expand_v13_digest_table(v13)


if __name__ == '__main__':
    unittest.main()
