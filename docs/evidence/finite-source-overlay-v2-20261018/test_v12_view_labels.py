"""Lossless V12 transport controls; no source admission or runtime claims."""
import copy
import hashlib
import json
from pathlib import Path
import runpy
import unittest
import overlay_page as codec

ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / 'src/source_reflows_merge/main_e177_overlay_labels_v12'
OLD = runpy.run_path(str(FIXTURE / 'controller.before.py.txt'))


# Closed current-source acquisition precedes every unchanged legacy assertion.
_FRONT_PATH = 'src/source_reflows/next_custody_fronts.json'
_FRONT_PIN = {'length': 1408488, 'sha256': '8f07ea37410b7c1901077be2f784a8fb4601efeda65f58fed311541a160b67c4'}
_NEXT_PATH = 'src/source_reflows/next_custody_inverse.json'
_PAIRS = [
    ('prelude286', 'tests/workspace/semantic_graph/source_inventory.rs'),
    ('v12-root', 'src/source_reflows_merge/main_e177_overlay_v2.rs'),
    ('v12-python', 'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py'),
    ('main295-v12-fragments', 'src/source_reflows_merge/main_e177_overlay_fragments_v3.rs'),
]

def _source_bytes(relative):
    import os, stat
    path = ROOT / relative
    for ancestor in (path, *path.parents):
        assert not stat.S_ISLNK(os.lstat(ancestor).st_mode)
    initial = os.lstat(path)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        identity = lambda s: (s.st_dev, s.st_ino, s.st_mode, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
        opened = os.fstat(fd)
        assert stat.S_ISREG(opened.st_mode) and identity(initial) == identity(opened)
        assert opened.st_size <= 2_097_152
        with os.fdopen(fd, 'rb', closefd=False) as stream:
            raw = stream.read(2_097_153)
        assert len(raw) == opened.st_size
        assert identity(opened) == identity(os.fstat(fd)) == identity(os.lstat(path))
        return raw
    finally:
        os.close(fd)

def _pin_bytes(raw, expected):
    assert len(raw) == expected['length']
    assert hashlib.sha256(raw).hexdigest() == expected['sha256']
    return raw

_FRONT_POOL_SCHEMA = 'semaprax.source-edit-string-pool.v1'
_FRONT_POOL_REF = '$source_edit_string_v1'
_FRONT_SOURCE_CAP = 9_753_094

def _front_pool_edits(document):
    assert document['schema'] == 'semaprax.next-custody-four-fronts.v1'
    for row in document['rows']:
        yield row['target']['edits']
        if row['middle'] is not None:
            yield row['middle']['edits']
    constructor = document['constructor_retention_903']
    assert constructor['schema'] == 'semaprax.constructor-retention-903-source-fronts.v1'
    for source in sorted(constructor['records']):
        yield constructor['records'][source]['edits']
    main8 = document['main8_raw_current_fronts']
    assert main8['schema'] == 'semaprax.main5b4.current-raw-input-fronts.v1'
    for row in main8['rows']:
        yield row['inverse']

def _front_pool_slots(document):
    for group in _front_pool_edits(document):
        assert type(group) is list
        for edit in group:
            if type(edit) is dict and set(edit) == {'start', 'end', 'after', 'before'}:
                for key in ('after', 'before'):
                    yield edit, key

def _front_json_size(value):
    # Count canonical UTF-8 output without forming an expanded serialization.
    if type(value) is str:
        size = 2
        for char in value:
            size += (2 if char in '\\"\b\f\n\r\t' else
                     6 if ord(char) < 32 else len(char.encode()))
        return size
    if value is None:
        return 4
    if type(value) is bool:
        return 4 if value else 5
    if type(value) in (int, float):
        return len(json.dumps(value, allow_nan=False).encode())
    if type(value) is list:
        return 2 + max(0, len(value) - 1) + sum(_front_json_size(v) for v in value)
    assert type(value) is dict and all(type(k) is str for k in value)
    return 2 + max(0, len(value) - 1) + sum(
        _front_json_size(k) + 1 + _front_json_size(v) for k, v in value.items())

def _front_unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        assert key not in result, 'duplicate source-front key'
        result[key] = value
    return result

def _front_finite_float(text):
    value = float(text)
    assert value == value and abs(value) != float('inf'), 'nonfinite JSON number'
    return value

def _front_invalid_constant(_text):
    raise AssertionError('nonfinite JSON constant')

def _front_integer(text):
    # serde_json represents JSON -0 as a floating negative zero, which cannot
    # impersonate the nonnegative integer required in a reference slot.
    return -0.0 if text == '-0' else int(text)

def _front_scalar_text(value):
    # serde_json accepts Unicode scalar text, not lone surrogate code units.
    if type(value) is str:
        try:
            value.encode()
        except UnicodeEncodeError:
            raise AssertionError('non-scalar JSON text') from None
    elif type(value) is dict:
        for key, child in value.items():
            _front_scalar_text(key)
            _front_scalar_text(child)
    elif type(value) is list:
        for child in value:
            _front_scalar_text(child)

def _front_body_size(raw):
    # Full JSON validation precedes this lexical scan. Preserve opaque number
    # spellings rather than counting a language-specific float serialization.
    whitespace = b' \t\r\n'
    def skip(pos):
        while pos < len(raw) and raw[pos] in whitespace:
            pos += 1
        return pos
    def string_end(pos):
        assert raw[pos] == 34
        pos += 1
        while raw[pos] != 34:
            pos += 2 if raw[pos] == 92 else 1
        return pos + 1
    def value_end(pos):
        if raw[pos] == 34:
            return string_end(pos)
        if raw[pos] not in b'[{':
            while pos < len(raw) and raw[pos] not in b',}] \t\r\n':
                pos += 1
            return pos
        depth = 0
        while pos < len(raw):
            char = raw[pos]
            if char == 34:
                pos = string_end(pos)
                continue
            depth += (1 if char in b'[{' else -1 if char in b']}' else 0)
            pos += 1
            if depth == 0:
                return pos
        raise AssertionError('incomplete body')
    pos = skip(0)
    assert raw[pos] == 123
    pos = skip(pos + 1)
    while raw[pos] != 125:
        end = string_end(pos)
        key = json.loads(raw[pos:end])
        pos = skip(end)
        assert raw[pos] == 58
        pos = skip(pos + 1)
        end = value_end(pos)
        if key == 'body':
            return end - pos
        pos = skip(end)
        if raw[pos] == 44:
            pos = skip(pos + 1)
    raise AssertionError('missing body')

def _front_expand_pool(envelope, body_size):
    assert set(envelope) == {'schema', 'strings', 'body'}
    assert envelope['schema'] == _FRONT_POOL_SCHEMA
    strings = envelope['strings']
    assert type(strings) is list and all(type(s) is str and len(s.encode()) >= 64 for s in strings)
    assert len(set(strings)) == len(strings), 'duplicate pool string'
    document = envelope['body']
    assert type(document) is dict
    uses = [0] * len(strings)
    first = 0
    literals = {}
    declared = set(strings)
    expanded_size = body_size + 1
    # Borrowed validation and complete accounting precede every substitution.
    for edit, key in _front_pool_slots(document):
        value = edit[key]
        if type(value) is str:
            assert value not in declared, 'literal/reference alias'
            if len(value.encode()) >= 64:
                literals[value] = literals.get(value, 0) + 1
            continue
        assert type(value) is dict and set(value) == {_FRONT_POOL_REF}
        index = value[_FRONT_POOL_REF]
        assert type(index) is int and 0 <= index < len(strings)
        if uses[index] == 0:
            assert index == first, 'noncanonical first use'
            first += 1
        uses[index] += 1
        expanded_size += _front_json_size(strings[index]) - _front_json_size(value)
    assert all(n >= 2 for n in uses), 'unused or singleton declaration'
    assert all(n == 1 for n in literals.values()), 'repeated uninterned literal'
    assert expanded_size <= _FRONT_SOURCE_CAP, 'expanded source-front limit'
    for edit, key in _front_pool_slots(document):
        if type(edit[key]) is dict:
            edit[key] = strings[edit[key][_FRONT_POOL_REF]]
    return document

def _front_decode_bytes(raw):
    assert type(raw) is bytes and len(raw) <= 2_097_152
    # Preserve the independently executable legacy parser and return shape.
    value = json.loads(raw)
    if type(value) is dict and value.get('schema') == _FRONT_POOL_SCHEMA:
        value = json.loads(raw, object_pairs_hook=_front_unique_pairs,
                           parse_float=_front_finite_float, parse_int=_front_integer,
                           parse_constant=_front_invalid_constant)
        _front_scalar_text(value)
        return _front_expand_pool(value, _front_body_size(raw))
    assert type(value) is dict and value.get('schema') == 'semaprax.next-custody-four-fronts.v1'
    return value

def _front_encode_pool(document):
    # This is a pure format operation, never a source admission or file writer.
    assert _front_json_size(document) + 1 <= _FRONT_SOURCE_CAP
    counts = {}
    for edit, key in _front_pool_slots(document):
        value = edit[key]
        assert type(value) is str
        if len(value.encode()) >= 64:
            counts[value] = counts.get(value, 0) + 1
    strings = [value for value, count in counts.items() if count >= 2]
    indices = {value: index for index, value in enumerate(strings)}
    body = copy.deepcopy(document)
    for edit, key in _front_pool_slots(body):
        if edit[key] in indices:
            edit[key] = {_FRONT_POOL_REF: indices[edit[key]]}
    envelope = dict(schema=_FRONT_POOL_SCHEMA, strings=strings, body=body)
    assert _front_json_size(envelope) + 1 <= 2_097_152
    wire = json.dumps(envelope, separators=(',', ':'), ensure_ascii=False, allow_nan=False).encode() + b'\n'
    assert _front_decode_bytes(wire) == document
    return wire

def _front_data():
    raw = _pin_bytes(_source_bytes(_FRONT_PATH), _FRONT_PIN)
    data = _front_decode_bytes(raw)
    assert data['schema'] == 'semaprax.next-custody-four-fronts.v1'
    assert [(r['owner'], r['source']) for r in data['rows']] == _PAIRS
    legacy_raw = _pin_bytes(_source_bytes(_NEXT_PATH), data['original_packet'])
    legacy = json.loads(legacy_raw)
    assert len(legacy['owners']) == 26
    return raw, data, legacy_raw, legacy

def _front_project(actual, stage):
    prior_end = 0
    for edit in stage['edits']:
        start, end = edit['start'], edit['end']
        assert type(start) is int and type(end) is int
        assert prior_end <= start <= end <= len(actual)
        assert actual[start:end] == edit['after'].encode()
        actual[:start].decode(); actual[:end].decode()
        prior_end = end
    result = actual
    for edit in reversed(stage['edits']):
        result = result[:edit['start']] + edit['before'].encode() + result[edit['end']:]
    return _pin_bytes(result, stage)

def _acquire_next_current(owner, source, supplied):
    assert (owner, source) in _PAIRS
    front_raw, front, legacy_raw, legacy = _front_data()
    entry = front['rows'][_PAIRS.index((owner, source))]
    actual = _pin_bytes(_source_bytes(source), entry)
    assert supplied.encode() == actual, 'Python acquisition requires the actual physical source'
    row, = [r for r in legacy['owners'] if (r['owner'], r['source']) == (owner, source)]
    assert entry['target']['length'] == row['current_length']
    assert entry['target']['sha256'] == row['current_sha256']
    result = _front_project(actual, entry['target'])
    assert _source_bytes(source) == actual
    assert _source_bytes(_FRONT_PATH) == front_raw and _source_bytes(_NEXT_PATH) == legacy_raw
    return result.decode()



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
        # Exercise every stale front without changing this owning test count.
        front_raw, fronts, legacy_raw, legacy = _front_data()
        for owner, source in _PAIRS:
            physical = _source_bytes(source).decode()
            acquired = _acquire_next_current(owner, source, physical)
            row, = [r for r in legacy['owners'] if (r['owner'], r['source']) == (owner, source)]
            raw = acquired.encode()
            self.assertEqual(len(raw), row['current_length'])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), row['current_sha256'])
            for edit in reversed(row['edits']):
                self.assertEqual(raw[edit['start']:edit['end']], edit['after'].encode())
                raw = raw[:edit['start']] + edit['before'].encode() + raw[edit['end']:]
            self.assertEqual(raw, row['before_text'].encode())
            self.assertEqual(len(raw), row['before_length'])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), row['before_sha256'])
            for bad in ('', physical[:-1], physical + '\n', acquired):
                with self.subTest(owner=owner, refusal='foreign-or-stale-source'):
                    with self.assertRaises(AssertionError):
                        _acquire_next_current(owner, source, bad)
        with self.assertRaises(AssertionError):
            _acquire_next_current('foreign-owner', _PAIRS[0][1], '')
        before = (FIXTURE / 'controller.before.py.txt').read_bytes()
        self.assertEqual(hashlib.sha256(before).hexdigest(),
                         '5834ed5afd8ddaf3be4229b2a3a97edf2420b92b46ee195e44d48ea8844c71ec')
        current = (Path(__file__).parent / 'overlay_page.py').read_text()
        current = _acquire_next_current('v12-python', 'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py', current)
        packet = json.loads((ROOT / 'src/source_reflows/next_custody_inverse.json').read_text())
        row = next(row for row in packet['owners'] if row['owner'] == 'v12-python')
        self.assertEqual(len(current.encode()), row['current_length'])
        self.assertEqual(hashlib.sha256(current.encode()).hexdigest(), row['current_sha256'])
        raw = current.encode()
        for edit in reversed(row['edits']):
            self.assertEqual(raw[edit['start']:edit['end']], edit['after'].encode())
            raw = raw[:edit['start']] + edit['before'].encode() + raw[edit['end']:]
        self.assertEqual(raw, row['before_text'].encode())
        self.assertEqual(hashlib.sha256(raw).hexdigest(), row['before_sha256'])
        current = raw.decode()
        start = current.index("V12_SCHEMA = 'semaprax.finite-source-overlay-page.v12'\n")
        end = current.index('def compose_values(before, policy, page):\n', start)
        restored = (current[:start] + current[end:]).replace(
            " if page['schema']==V12_SCHEMA:page=expand_v12_view_labels(page)\n", '', 1)
        self.assertEqual(restored.encode(), before)
        self.assertEqual(codec.PAGE_CAP, OLD['PAGE_CAP'])
        self.assertEqual(codec.SOURCE_CAP, OLD['SOURCE_CAP'])
        self.assertEqual(_source_bytes(_FRONT_PATH), front_raw)
        self.assertEqual(_source_bytes(_NEXT_PATH), legacy_raw)
        for owner, source in _PAIRS:
            _acquire_next_current(owner, source, _source_bytes(source).decode())


class SourceFrontPoolTests(unittest.TestCase):
    @staticmethod
    def fixture():
        payload = 'é\\\"\x00' * 32
        edit = dict(start=0, end=0, after=payload, before=payload)
        return dict(
            schema='semaprax.next-custody-four-fronts.v1',
            rows=[dict(target=dict(edits=[edit]), middle=None)],
            constructor_retention_903=dict(
                schema='semaprax.constructor-retention-903-source-fronts.v1', records={}),
            main8_raw_current_fronts=dict(
                schema='semaprax.main5b4.current-raw-input-fronts.v1', rows=[]),
            opaque={_FRONT_POOL_REF: 900, 'float': 1e-7})

    def test_pool_whole_inverse_and_opaque_unknowns(self):
        document = self.fixture()
        before = json.dumps(document, separators=(',', ':'), ensure_ascii=False).encode()
        wire = _front_encode_pool(document)
        actual = _front_decode_bytes(wire)
        self.assertEqual(actual, document)
        self.assertEqual(json.dumps(actual, separators=(',', ':'), ensure_ascii=False).encode(), before)
        self.assertEqual(_front_decode_bytes(before), document)
        self.assertEqual(actual['opaque'], document['opaque'])
        self.assertEqual(_front_json_size(document), len(before))
        unknown = dict(start=0, end=0, after={_FRONT_POOL_REF: 8}, before='x', unknown=True)
        document['rows'][0]['target']['edits'].append(unknown)
        self.assertEqual(_front_decode_bytes(_front_encode_pool(document)), document)

    def test_pool_rejects_malformed_aliases_and_duplicates(self):
        envelope = json.loads(_front_encode_pool(self.fixture()))
        bads = []
        for index in (-1, True, 0.0, 1, '0', None):
            bad = copy.deepcopy(envelope)
            bad['body']['rows'][0]['target']['edits'][0]['after'] = {_FRONT_POOL_REF: index}
            bads.append(bad)
        for field, value in (('strings', []), ('strings', envelope['strings'] * 2),
                             ('strings', [64]), ('strings', ['short']),
                             ('schema', 'semaprax.source-edit-string-pool.unknown')):
            bad = copy.deepcopy(envelope)
            bad[field] = value
            bads.append(bad)
        bad = copy.deepcopy(envelope)
        bad['body']['rows'][0]['target']['edits'][0]['after'] = envelope['strings'][0]
        bads.append(bad)
        bad = copy.deepcopy(envelope)
        bad['unknown'] = True
        bads.append(bad)
        for bad in bads:
            with self.subTest(bad=repr(bad)[:120]), self.assertRaises((AssertionError, KeyError, TypeError)):
                _front_decode_bytes(json.dumps(bad).encode())
        raw = _front_encode_pool(self.fixture())
        negative_zero = raw.replace(b'"$source_edit_string_v1":0', b'"$source_edit_string_v1":-0', 1)
        self.assertNotEqual(negative_zero, raw)
        with self.assertRaises(AssertionError):
            _front_decode_bytes(negative_zero)
        duplicate = raw.replace(b'"schema":', b'"schema":"duplicate","schema":', 1)
        with self.assertRaises(AssertionError):
            _front_decode_bytes(duplicate)
        for scalar in (b'NaN', b'Infinity', b'-Infinity', b'1e999', b'"\\ud800"'):
            bad = raw.replace(b'1e-07', scalar)
            with self.subTest(scalar=scalar), self.assertRaises(AssertionError):
                _front_decode_bytes(bad)
        for field in ('constructor_retention_903', 'main8_raw_current_fronts'):
            bad = copy.deepcopy(envelope)
            bad['body'][field]['schema'] = 'unknown'
            with self.assertRaises(AssertionError):
                _front_decode_bytes(json.dumps(bad).encode())

    def test_pool_literal_repeat_and_first_use_are_canonical(self):
        document = self.fixture()
        envelope = json.loads(_front_encode_pool(document))
        envelope['body']['rows'][0]['target']['edits'].append(
            dict(start=0, end=0, after='y' * 64, before='y' * 64))
        with self.assertRaises(AssertionError):
            _front_decode_bytes(json.dumps(envelope).encode())
        wire = _front_encode_pool(envelope['body'] | {'rows': document['rows']})
        self.assertEqual(_front_decode_bytes(wire), document)
        document['rows'][0]['target']['edits'].append(
            dict(start=0, end=0, after='z' * 64, before='z' * 64))
        envelope = json.loads(_front_encode_pool(document))
        envelope['strings'].reverse()
        for edit in envelope['body']['rows'][0]['target']['edits']:
            for key in ('after', 'before'):
                edit[key][_FRONT_POOL_REF] = 1 - edit[key][_FRONT_POOL_REF]
        with self.assertRaises(AssertionError):
            _front_decode_bytes(json.dumps(envelope).encode())

    def test_pool_raw_body_numbers_and_cap_precede_expansion(self):
        from unittest.mock import patch
        envelope = json.loads(_front_encode_pool(self.fixture()))
        raw = json.dumps(envelope, ensure_ascii=False, indent=2).encode()
        raw = raw.replace(b'1e-07', b'1.000000000e-7')
        body_size = _front_body_size(raw)
        self.assertGreater(body_size, _front_json_size(envelope['body']))
        self.assertEqual(_front_decode_bytes(raw)['opaque']['float'], 1e-7)
        bound = body_size + 1
        for edit, key in _front_pool_slots(envelope['body']):
            index = edit[key][_FRONT_POOL_REF]
            bound += _front_json_size(envelope['strings'][index]) - _front_json_size(edit[key])
        with patch.dict(globals(), _FRONT_SOURCE_CAP=bound):
            self.assertEqual(_front_decode_bytes(raw), self.fixture())
        with patch.dict(globals(), _FRONT_SOURCE_CAP=bound - 1):
            with self.assertRaises(AssertionError):
                _front_decode_bytes(raw)
        # A small legal wire cannot amplify past the original logical cap.
        envelope['strings'] = ['x' * 65536]
        edit = dict(start=0, end=0, after={_FRONT_POOL_REF: 0}, before={_FRONT_POOL_REF: 0})
        envelope['body']['rows'][0]['target']['edits'] = [copy.deepcopy(edit) for _ in range(80)]
        raw = json.dumps(envelope, separators=(',', ':')).encode()
        self.assertLess(len(raw), 2_097_152)
        with self.assertRaises(AssertionError):
            _front_decode_bytes(raw)
        self.assertIsInstance(envelope['body']['rows'][0]['target']['edits'][0]['after'], dict)
        with self.assertRaises(AssertionError):
            _front_decode_bytes(b' ' * 2_097_153)


class PositionalSourceRows(unittest.TestCase):
    def fixture(self):
        known = {'source': 'literal.rs', 'compiled': False,
                 'current': [7, 'A' * 43], 'views': []}
        fragment = {**copy.deepcopy(known), 'compiled': True,
                    'fragments': {'source_row': [None]}}
        outer = {**copy.deepcopy(known), 'fragments': [fragment,
                 {'canonical_source': 'opaque', 'source_row': [False]}]}
        unknown = {**copy.deepcopy(known), 'unknown': {'source_row': [None]},
                   'source_row': 42}
        full = {'schema': codec.V12_SCHEMA, 'index': 1, 'total_pages': 1,
                'predecessor': 'kept', 'archive_prefixes': [], 'view_labels': [],
                'replacements': [{'old519_index': [0, [7, 'A' * 43], [],
                                  [copy.deepcopy(fragment)]]}],
                'additions': [outer, unknown]}
        return codec._encode_v15_logical_source_paths(codec._encode_v14_logical_pin_tails(
            codec._encode_v13_logical_digest_table(full)))

    def test_entire_inverse_and_unknown_one_level_metadata(self):
        before = self.fixture()
        saved = copy.deepcopy(before)
        legacy = codec.encode_v16_source_rows(before)
        wire = codec.encode_v17_source_rows(before)
        self.assertEqual(before, saved)
        self.assertEqual(codec.expand_v17_source_rows(wire), before)
        self.assertLess(len(codec.canonical(wire)), len(codec.canonical(legacy)))
        self.assertEqual(wire['additions'][0][1], 0)
        self.assertIs(type(wire['additions'][0][1]), int)
        self.assertEqual(wire['additions'][0][4][0][1], 1)
        self.assertIs(type(wire['additions'][0][4][0][1]), int)
        self.assertEqual(wire['additions'][1], before['additions'][1])
        self.assertEqual(wire['additions'][0][4][0][4], {'source_row': [None]})
        self.assertEqual(wire['additions'][0][4][1], before['additions'][0]['fragments'][1])
        self.assertEqual(codec.encode_v16_source_rows(before), legacy)

    def test_hostile_arity_types_aliases_and_fragment_domain(self):
        wire = codec.encode_v17_source_rows(self.fixture())
        bad = [[], [None] * 4, ['literal.rs', 2, [], []],
               ['literal.rs', False, [], []], ['literal.rs', 1.0, [], []],
               ['literal.rs', 1, None, []], ['literal.rs', 1, [], {}],
               {'source_row': ['literal.rs', True, [], []]},
               {'source': 'literal.rs', 'compiled': True, 'current': [], 'views': []},
               ['literal.rs', 1, [], [], {'opaque': True}]]
        for replacement in bad:
            with self.subTest(replacement=replacement):
                changed = copy.deepcopy(wire)
                changed['additions'][0] = replacement
                with self.assertRaises(AssertionError):
                    codec.expand_v17_source_rows(changed)
        raw = codec.canonical(wire)
        negative_zero = raw.replace(b'["literal.rs",0,', b'["literal.rs",-0,', 1)
        self.assertNotEqual(negative_zero, raw)
        with self.assertRaisesRegex(AssertionError, 'noncanonical JSON'):
            codec.parse(negative_zero)
        changed = copy.deepcopy(wire)
        changed['replacements'][0]['old519_index'] = [0, [], []]
        with self.assertRaises(AssertionError):
            codec.expand_v17_source_rows(changed)

    def test_bounds_refuse_before_payload_clone(self):
        wire = codec.encode_v17_source_rows(self.fixture())
        old_cap = codec.SOURCE_CAP
        old_copy = codec.copy.deepcopy
        try:
            codec.SOURCE_CAP = len(codec.canonical(wire))
            def forbidden_clone(_):
                raise RuntimeError('payload cloned before bound')
            codec.copy.deepcopy = forbidden_clone
            with self.assertRaisesRegex(AssertionError, 'v17 expanded bound'):
                codec.expand_v17_source_rows(wire)
            codec.SOURCE_CAP = old_cap
            wire['predecessor'] = 'x' * codec.PAGE_CAP
            with self.assertRaisesRegex(AssertionError, 'v17 same one-page cap'):
                codec.expand_v17_source_rows(wire)
        finally:
            codec.SOURCE_CAP = old_cap
            codec.copy.deepcopy = old_copy



class V11EnvelopeCompatibility(unittest.TestCase):
    def test_entire_frozen_digest_and_named_view_v10_preimages_remain_independent(self):
        digest = codec.parse((FIXTURE / 'v11.before.json').read_bytes())
        self.assertEqual(codec.canonical(codec.expand_v11_compatible(digest)),
                         codec.canonical(codec.expand_v11_digest_pins(digest)))
        sha = '2018302f513448433f418a128169982a6bbe55652a843397e37fc5ed9ab9ac64'
        raw = (ROOT / 'e' / sha).read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), sha)
        v10 = codec.parse(raw)
        compact = codec.encode_v11_names(v10)
        self.assertEqual(codec.canonical(codec.expand_v11_compatible(compact)), raw)
        self.assertEqual(compact['schema'], digest['schema'])
        self.assertNotEqual(set(compact), set(digest))

    def test_original_named_slots_preserve_opaque_objects_and_nested_values(self):
        old = {'schema': codec.V10_SCHEMA, 'replacements': [], 'additions': [{
            'source': 'opaque', 'compiled': True, 'current': [1, 'f' * 64],
            'views': [['same', 1, 'f' * 64], ['same', 1, 'f' * 64],
                      {'name': 7, 'unknown': {'names': [0], 'page': False}}],
            'unknown': {'names': [0], 'page': {'schema': codec.V11_SCHEMA}}}]}
        compact = codec.encode_v11_names(old)
        self.assertEqual(compact['names'], ['same'])
        self.assertEqual(codec.expand_v11_compatible(compact), old)
        self.assertEqual(compact['page']['additions'][0]['unknown'],
                         old['additions'][0]['unknown'])

    def test_public_v10_forms_and_named_v11_keep_exact_legacy_provenance(self):
        before = {'schema': 'fixture', 'source_cap': codec.SOURCE_CAP,
                  'opaque': {'names': [0], 'archive_prefixes': False},
                  'rows': [{'source': f'src/provenance_{i}.rs', 'compiled': False,
                            'current': {'length': 0, 'sha256': '0' * 64,
                                        'archive': 'e/' + '0' * 64}, 'views': []}
                           for i in range(519)]}
        predecessor = {'rows': 519, 'logical_sha256': codec.H(codec.canonical(before))}
        policy = {'schema': 'semaprax.finite-source-overlay-policy.v2',
                  'page': {'path': codec.PAGE_PATH, 'length': 0, 'sha256': '0' * 64},
                  'predecessor': predecessor, 'replacements': [], 'additions': [],
                  'new_fragment_edges': []}
        policy['replacements'] = [row['source'] for row in before['rows'][:2]]
        logical = {'schema': codec.V9_SCHEMA, 'index': 1, 'total_pages': 1,
                   'predecessor': predecessor, 'replacements': [
                       {'source': row['source'], 'compiled': row['compiled'],
                        'current': [0, '0' * 64, 'e/' + '0' * 64], 'views': []}
                       for row in before['rows'][:2]], 'additions': []}
        legacy = codec.encode_v10_pins(before, policy, logical)
        prefix = codec.encode_v10_with_prefixes(logical, ['e/'])
        unused = dict(prefix, replacements=[])
        with self.assertRaisesRegex(AssertionError, 'v10 unused archive prefix'):
            codec.compose_values(before, policy, unused)
        for value in (legacy, prefix):
            self.assertEqual(codec.compose_values(before, policy, value), before)
        named = {'schema': codec.V11_SCHEMA, 'names': [], 'page': legacy}
        self.assertEqual(codec.compose_values(before, policy, named), before)
        # The prefix shape is genuinely valid above. Wrapping it in names must
        # not select that different codec: the original named route is legacy.
        crossed = dict(named, page=prefix)
        with self.assertRaises(AssertionError):
            codec.compose_values(before, policy, crossed)
        for value in (legacy, prefix):
            for key in ('names', 'page', 'unknown'):
                changed = dict(value); changed[key] = None
                with self.assertRaises(AssertionError):
                    codec.compose_values(before, policy, changed)

    def test_mixed_unknown_and_missing_outer_keys_refuse_before_payload_decoders(self):
        direct, _ = ViewLabels().pair()
        named = {'schema': codec.V11_SCHEMA, 'names': [], 'page': {}}
        hostile = []
        for original in (direct, named):
            changed = copy.deepcopy(original); changed['unknown'] = 0; hostile.append(changed)
            for key in original:
                changed = copy.deepcopy(original); del changed[key]; hostile.append(changed)
        for key in ('names', 'page'):
            changed = copy.deepcopy(direct); changed[key] = named[key]; hostile.append(changed)
        changed = copy.deepcopy(named); changed['replacements'] = []; hostile.append(changed)
        old_digest, old_names = codec.expand_v11_digest_pins, codec.expand_v11_names
        def forbidden(_):
            raise RuntimeError('payload decoder reached before envelope refusal')
        try:
            codec.expand_v11_digest_pins = forbidden
            codec.expand_v11_names = forbidden
            for value in hostile:
                with self.assertRaises(AssertionError):
                    codec.expand_v11_compatible(value)
        finally:
            codec.expand_v11_digest_pins = old_digest
            codec.expand_v11_names = old_names



class TypedFragmentReferences(unittest.TestCase):
    def fixture(self):
        value = PositionalSourceRows().fixture()
        value['replacements'][0]['old519_index'][3] = [
            {'canonical_source': 519}, {'canonical_source': 520},
            {'canonical_source': 'opaque', 'unknown': {'canonical_source': 519}}]
        value['additions'][1]['opaque_reference'] = {'canonical_source': 519}
        return value

    def test_entire_inverse_order_unknowns_and_unchanged_v17(self):
        before = self.fixture()
        old = codec.encode_v17_source_rows(before)
        wire = codec.encode_v18_fragment_refs(before)
        self.assertEqual(codec.expand_v18_fragment_refs(wire), before)
        self.assertEqual(codec.encode_v17_source_rows(before), old)
        self.assertEqual(wire['replacements'][0]['old519_index'][3][:2], [519, 520])
        self.assertEqual(len(codec.canonical(old))-len(codec.canonical(wire)), 42)
        self.assertEqual(wire['additions'][1]['opaque_reference'], {'canonical_source': 519})
        before['additions'][1]['fragments'] = [{
            'source': 'nested.rs', 'compiled': True, 'current': [7, 'A'*43], 'views': [],
            'fragments': [519, {'canonical_source': 519}]}]
        with self.assertRaisesRegex(AssertionError, 'v13 uninterned or aliased digest'):
            codec.encode_v18_fragment_refs(before)
        before['additions'][1]['fragments'][0]['current'][1] = 0
        with self.assertRaisesRegex(AssertionError, 'v14 uninterned or aliased tail'):
            codec.encode_v18_fragment_refs(before)
        admitted_tail = copy.deepcopy(before['replacements'][0]['old519_index'][1])
        self.assertEqual(len(admitted_tail), 1)
        self.assertIs(type(admitted_tail[0]), int)
        before['additions'][1]['fragments'][0]['current'] = admitted_tail
        self.assertEqual(codec.expand_v18_fragment_refs(codec.encode_v18_fragment_refs(before)), before)

    def test_hostile_types_aliases_wrong_slots_and_truncation(self):
        wire = codec.encode_v18_fragment_refs(self.fixture())
        for bad in [True, False, -1, 0, 518, 519.0, -0.0,
                    519+len(wire['additions']), {'canonical_source': 519}]:
            changed = copy.deepcopy(wire)
            changed['replacements'][0]['old519_index'][3][0] = bad
            with self.assertRaises(AssertionError):codec.expand_v18_fragment_refs(changed)
        for path, bad in [('additions', [519]), ('replacements', [{'old519_index': [0, [], []]}])]:
            changed = copy.deepcopy(wire);changed[path] = bad
            with self.assertRaises(AssertionError):codec.expand_v18_fragment_refs(changed)
        changed = copy.deepcopy(wire);changed['unknown'] = 519
        with self.assertRaises(AssertionError):codec.expand_v18_fragment_refs(changed)
        raw = codec.canonical(wire);bad = raw.replace(b'[519,520,', b'[-0,520,', 1)
        self.assertNotEqual(bad, raw)
        with self.assertRaises(AssertionError):codec.parse(bad)

    def test_exact_short_bound_precedes_clone_and_wire_cap(self):
        before = self.fixture();wire = codec.encode_v18_fragment_refs(before)
        original_cap, original_copy = codec.SOURCE_CAP, codec.copy.deepcopy
        try:
            exact = len(codec.canonical(before))
            self.assertEqual(codec._v18_preflight(wire, exact), exact)
            with self.assertRaisesRegex(AssertionError, 'v18 expanded bound'):
                codec._v18_preflight(wire, exact-1)
            codec.SOURCE_CAP = exact-1
            def forbidden(_):raise RuntimeError('expanded payload cloned before full bound')
            codec.copy.deepcopy = forbidden
            with self.assertRaisesRegex(AssertionError, 'v18 expanded bound'):
                codec.expand_v18_fragment_refs(wire)
            codec.SOURCE_CAP = original_cap
            wire['predecessor'] = 'x' * codec.PAGE_CAP
            with self.assertRaisesRegex(AssertionError, 'v18 same one-page cap'):
                codec.expand_v18_fragment_refs(wire)
        finally:codec.SOURCE_CAP, codec.copy.deepcopy = original_cap, original_copy

    def test_current_real_page_keeps_entire_v15_preimage(self):
        current = codec.parse((Path(__file__).parent/'page.json').read_bytes())
        if current['schema']==codec.V20_SCHEMA:
            codec._v20_successor(current);current=current['page']
            before=codec.expand_v20_compact_fragment_refs(current)
            self.assertEqual(codec.encode_v20_compact_rows(before),current)
            with self.assertRaises(AssertionError):codec.expand_v18_fragment_refs(current)
            return
        if current['schema']==codec.V19_SCHEMA:
            codec._v19_successor(current); current = current['page']
        if current['schema']==codec.V19_INDEXED_SCHEMA:
            before = codec.expand_v19_indexed_fragment_refs(current)
            wire = codec.encode_v19_indexed_inner(before)
            self.assertEqual(codec.expand_v19_indexed_fragment_refs(wire), before)
            self.assertEqual(wire, current)
            self.assertLessEqual(len(codec.canonical(wire)), codec.PAGE_CAP)
            # This actual complete batch exceeds the old physical format's
            # cap. Public V18 must still refuse rather than inherit a bypass.
            with self.assertRaisesRegex(AssertionError, 'v18 same one-page cap'):
                codec.encode_v18_fragment_refs(before)
            return
        before = (codec.expand_v18_fragment_refs(current) if current['schema']==codec.V18_SCHEMA
                  else codec.expand_v17_source_rows(current))
        wire = codec.encode_v18_fragment_refs(before)
        self.assertEqual(codec.expand_v18_fragment_refs(wire), before)
        self.assertLessEqual(len(codec.canonical(wire)), codec.PAGE_CAP)
        if current['schema']==codec.V18_SCHEMA:self.assertEqual(wire, current)

class CompiledSuccessorV19(unittest.TestCase):
    @staticmethod
    def fixture(compact=False, indexed=False):
        def current(raw, immutable=True):
            digest = hashlib.sha256(raw).hexdigest()
            return {'length': len(raw), 'sha256': digest, 'archive': ('e/' if immutable else 'fixture/') + digest}
        declaration = codec._V19_DECLARATION.encode()
        owner_raw, source_raw = b'\n\n' + declaration + b'\n', b'compiled current fixture\n'
        rows = [{'source': f'src/fixture/item_{i}.rs', 'compiled': False,
                 'current': current(b'old', immutable=False), 'views': [], 'opaque': {'order': [3, 1, 2]}}
                for i in range(519)]
        rows[0].update(source=codec._V19_OWNER, compiled=True, current=current(owner_raw))
        rows[10].update(source=sorted(codec.V3_ROOTS)[0], compiled=True)
        rows[389]['source'] = codec._V19_SOURCE
        before = {'schema': 'fixture', 'source_cap': codec.SOURCE_CAP,
                  'rows': rows, 'opaque_root': [False, {'keep': None}]}
        target = copy.deepcopy(rows[389]); target['compiled'] = True
        target['current'] = current(source_raw)
        target['views'].append({'name': 'whole-pre-overlay-v2-current', **rows[389]['current']})
        carrier = copy.deepcopy(rows[10]); carrier['fragments'] = [copy.deepcopy(target)]
        predecessor = {'rows': 519, 'logical_sha256': codec.H(codec.canonical(before)), 'extension': {}}
        policy = {'schema': 'semaprax.finite-source-overlay-policy.v2', 'page': {},
                  'predecessor': predecessor, 'replacements': [carrier['source'], target['source']],
                  'additions': [], 'new_fragment_edges': [{'parent': carrier['source'], 'child': target['source']}]}
        logical = {'schema': codec.V8_SCHEMA, 'index': 1, 'total_pages': 1,
                   'predecessor': copy.deepcopy(predecessor), 'replacements': [carrier, target], 'additions': []}
        if indexed:
            logical['replacements'] = [{'old519_index': [index, row['current'], row['views'], row.get('fragments')]} for index, row in zip((10, 389), logical['replacements'])]
        v9 = codec.transform_v9_pin_slots(logical, True); v9['schema'] = codec.V9_SCHEMA
        v10 = codec.encode_v10_with_prefixes(v9, ['fixture/'])
        v11 = codec.transform_v11_digest_slots(v10, True); v11['schema'] = codec.V11_SCHEMA
        inner = (codec.encode_v19_indexed_inner if indexed else codec.encode_v18)(codec.encode_v12_view_labels(v11))
        successor = {'source': codec._V19_SOURCE, 'prior_row_sha256': codec.H(codec.canonical(rows[389])),
                     'current': copy.deepcopy(target['current']), 'registration': {
                         'owner': codec._V19_OWNER, 'current': copy.deepcopy(rows[0]['current']),
                         'start': 2, 'text': codec._V19_DECLARATION}}
        wire = codec.encode_v19(inner, successor)
        assert codec.canonical(codec._v19_successor(wire)) == codec.canonical(successor)
        if not compact:wire['compiled_successor'] = copy.deepcopy(successor)
        return before, policy, wire, {codec._V19_OWNER: owner_raw, codec._V19_SOURCE: source_raw}

    def test_whole_original_inverse_and_new_compiled_fragment(self):
        before, policy, wire, _ = self.fixture(); original = codec.canonical(before)
        result = codec.compose_values(before, policy, wire)
        self.assertIs(result['rows'][389]['compiled'], True)
        self.assertEqual(result['rows'][10]['fragments'], [result['rows'][389]])
        self.assertEqual(result['opaque_root'], before['opaque_root'])
        self.assertEqual(result['rows'][389]['opaque'], before['rows'][389]['opaque'])
        self.assertEqual(codec.canonical(before), original)
        with self.assertRaises(AssertionError):codec.compose_values(before, policy, wire['page'])

    def test_singleton_unknown_false_positive_and_stale_witness_refusals(self):
        before, policy, wire, _ = self.fixture()
        mutations = [lambda p: p.update(extra=None),
                     lambda p: p.update(compiled_successor=[p['compiled_successor'], p['compiled_successor']]),
                     lambda p: p['compiled_successor'].update(extra=None),
                     lambda p: p['compiled_successor'].update(source='src/foreign.rs'),
                     lambda p: p['compiled_successor'].update(prior_row_sha256='0'*64),
                     lambda p: p['compiled_successor']['current'].update(sha256='0'*64, archive='e/'+'0'*64),
                     lambda p: p['compiled_successor']['current'].update(archive='arbitrary/normal/path'),
                     lambda p: p['compiled_successor']['registration'].update(extra=None),
                     lambda p: p['compiled_successor']['registration'].update(start=True),
                     lambda p: p['compiled_successor']['registration'].update(text='#[cfg(test)]\n'+codec._V19_DECLARATION),
                     lambda p: p['compiled_successor']['registration']['current'].update(sha256='0'*64, archive='e/'+'0'*64)]
        for mutate in mutations:
            bad = copy.deepcopy(wire); mutate(bad)
            with self.subTest(mutate=mutate), self.assertRaises(AssertionError):codec.compose_values(before, policy, bad)
        for replacements in ([policy['replacements'][0]], policy['replacements'] + [codec._V19_SOURCE]):
            bad_policy = copy.deepcopy(policy);bad_policy['replacements'] = replacements
            with self.assertRaisesRegex(AssertionError, 'v19 explicit replacement'):
                codec.compose_values(before, bad_policy, wire)
        bad_owner = copy.deepcopy(before);bad_owner['rows'][0]['compiled'] = False
        bad_policy = copy.deepcopy(policy);bad_policy['predecessor']['logical_sha256'] = codec.H(codec.canonical(bad_owner))
        bad_wire = copy.deepcopy(wire);bad_wire['page']['predecessor'] = copy.deepcopy(bad_policy['predecessor'])
        with self.assertRaisesRegex(AssertionError, 'v19 owner current'):
            codec.compose_values(bad_owner, bad_policy, bad_wire)
        bad_before = copy.deepcopy(before);bad_before['rows'][389]['compiled'] = True
        bad_policy = copy.deepcopy(policy);bad_policy['predecessor']['logical_sha256'] = codec.H(codec.canonical(bad_before))
        bad_wire = copy.deepcopy(wire);bad_wire['page']['predecessor'] = copy.deepcopy(bad_policy['predecessor'])
        with self.assertRaisesRegex(AssertionError, 'v19 entire old false row'):
            codec.compose_values(bad_before, bad_policy, bad_wire)

    def test_trusted_bindings_precede_every_read_and_physical_drift_refuses(self):
        _, policy, wire, raw = self.fixture(); calls = []
        pins = {s: {'length': len(b), 'sha256': hashlib.sha256(b).hexdigest()} for s, b in raw.items()}
        overlay = codec.Overlay(None, None, None, None, None, None, lambda s: pins[s])
        overlay.load = lambda: (policy, wire)
        def acquire(row):
            calls.append(row['source']); value = raw[row['source']]
            self.assertEqual({'length': len(value), 'sha256': hashlib.sha256(value).hexdigest()},
                             {k: row['current'][k] for k in ('length', 'sha256')})
            return value
        overlay.current = acquire
        self.assertEqual(overlay.load_current(), (policy, wire)); self.assertEqual(calls, [codec._V19_SOURCE, codec._V19_OWNER])
        calls.clear(); pins[codec._V19_SOURCE]['sha256'] = '0'*64
        with self.assertRaisesRegex(AssertionError, 'v19 stale trusted current binding'):overlay.load_current()
        self.assertFalse(calls)
        pins[codec._V19_SOURCE]['sha256'] = wire['compiled_successor']['current']['sha256']
        raw[codec._V19_OWNER] = b'x' + raw[codec._V19_OWNER][1:]
        with self.assertRaises(AssertionError):overlay.load_current()
        raw[codec._V19_OWNER] = b'\nX' + raw[codec._V19_OWNER][2:]
        pins[codec._V19_OWNER]['sha256'] = hashlib.sha256(raw[codec._V19_OWNER]).hexdigest()
        wire['compiled_successor']['registration']['current']['sha256'] = pins[codec._V19_OWNER]['sha256']
        wire['compiled_successor']['registration']['current']['archive'] = 'e/' + pins[codec._V19_OWNER]['sha256']
        with self.assertRaisesRegex(AssertionError, 'v19 declaration attribute boundary'):overlay.load_current()

    def test_compact_tuple_whole_object_inverse_and_hostile_shapes(self):
        fresh = runpy.run_path(str(Path(codec.__file__)), run_name='v19_actual_helper_namespace')
        self.assertIn('re', fresh)
        before, policy, compact, raw = self.fixture(compact=True)
        _, _, original, _ = self.fixture()
        self.assertEqual(fresh['encode_v19'](compact['page'], original['compiled_successor']), compact)
        self.assertEqual(fresh['canonical'](fresh['compose_values'](before, policy, compact)),
                         codec.canonical(codec.compose_values(before, policy, compact)))
        self.assertEqual(codec.canonical(codec._v19_successor(compact)), codec.canonical(original['compiled_successor']))
        self.assertEqual(codec.canonical(codec.compose_values(before, policy, compact)),
                         codec.canonical(codec.compose_values(before, policy, original)))
        self.assertEqual(compact['page'], original['page'])
        malformed = [[], compact['compiled_successor'][:-1], compact['compiled_successor'] + [None]]
        for index, values in [(0, ['other', True, None]), (1, [None, True, '0'*63, 'g'*64]),
                              (2, [True, 1.5, None, -1, codec.SOURCE_CAP+1]),
                              (3, [None, True, '0'*63, 'g'*64]),
                              (4, [True, 1.5, None, -1, codec.SOURCE_CAP+1]),
                              (5, [None, True, '0'*63, 'g'*64]),
                              (6, [True, 1.5, None, -1, codec.SOURCE_CAP+1])]:
            for value in values:
                record = copy.deepcopy(compact['compiled_successor']);record[index] = value;malformed.append(record)
        for record in malformed:
            bad = copy.deepcopy(compact);bad['compiled_successor'] = record
            with self.subTest(record=record), self.assertRaises(AssertionError):codec.compose_values(before, policy, bad)
        pins = {source: {'length': len(value), 'sha256': hashlib.sha256(value).hexdigest()} for source, value in raw.items()}
        calls = [];overlay = codec.Overlay(None, None, None, None, None, None, lambda source: pins[source])
        overlay.current = lambda row: calls.append(row['source']) or raw[row['source']]
        codec.authenticate_v19_registration(overlay, compact)
        self.assertEqual(calls, [codec._V19_SOURCE, codec._V19_OWNER]);calls.clear()
        pins[codec._V19_OWNER]['sha256'] = '0'*64
        with self.assertRaisesRegex(AssertionError, 'v19 stale trusted current binding'):
            codec.authenticate_v19_registration(overlay, compact)
        self.assertFalse(calls)

        indexed_before, indexed_policy, indexed_wire, _ = self.fixture(compact=True, indexed=True)
        indexed_result = codec.compose_values(indexed_before, indexed_policy, indexed_wire)
        old_before, old_policy, old_wire, _ = self.fixture(compact=True)
        self.assertEqual(codec.canonical(indexed_result), codec.canonical(codec.compose_values(old_before, old_policy, old_wire)))
        inner = indexed_wire['page']
        self.assertEqual(inner['schema'], codec.V19_INDEXED_SCHEMA)
        compact_slot = next(i for i, row in enumerate(inner['replacements']) if type(row) is list and type(row[0]) is int)
        for bad_index in (True, 1.0, -1, 519, '0'):
            hostile = copy.deepcopy(indexed_wire); hostile['page']['replacements'][compact_slot][0] = bad_index
            with self.subTest(indexed_reference=bad_index), self.assertRaises(AssertionError):
                codec.compose_values(indexed_before, indexed_policy, hostile)
        hostile = copy.deepcopy(indexed_wire)
        hostile['page']['replacements'][compact_slot] = {'old519_index': hostile['page']['replacements'][compact_slot]}
        with self.assertRaisesRegex(AssertionError, 'v19 indexed wrapped old519 alias'):
            codec.compose_values(indexed_before, indexed_policy, hostile)
        for group in ('additions', 'fragments'):
            hostile = copy.deepcopy(indexed_wire); value = copy.deepcopy(hostile['page']['replacements'][compact_slot])
            if group == 'additions': hostile['page']['additions'].append(value)
            else: hostile['page']['replacements'][compact_slot][3] = [value]
            with self.subTest(indexed_alias=group), self.assertRaises(AssertionError):
                codec.compose_values(indexed_before, indexed_policy, hostile)
        with self.assertRaises(AssertionError): codec.expand_v18_fragment_refs(inner)

    def test_wire_cap_refuses_before_clone(self):
        _, _, wire, _ = self.fixture(); wire['page']['opaque'] = 'x'*codec.PAGE_CAP
        original = codec.copy.deepcopy
        try:
            codec.copy.deepcopy = lambda _: (_ for _ in ()).throw(RuntimeError('cloned before cap'))
            with self.assertRaisesRegex(AssertionError, 'v19 same one-page cap'):codec._v19_successor(wire)
        finally:codec.copy.deepcopy = original



class _V20AuthorityCorpus:
    @staticmethod
    def fixture(compact=False, indexed=False):
        def current(raw, immutable=True):
            digest = hashlib.sha256(raw).hexdigest()
            return {'length': len(raw), 'sha256': digest, 'archive': ('e/' if immutable else 'fixture/') + digest}
        declaration = codec._V19_DECLARATION.encode()
        owner_raw, source_raw = b'\n\n' + declaration + b'\n', b'compiled current fixture\n'
        rows = [{'source': f'src/fixture/item_{i}.rs', 'compiled': False,
                 'current': current(b'old', immutable=False), 'views': [], 'opaque': {'order': [3, 1, 2]}}
                for i in range(519)]
        rows[0].update(source=codec._V19_OWNER, compiled=True, current=current(owner_raw))
        rows[10].update(source=sorted(codec.V3_ROOTS)[0], compiled=True)
        rows[389]['source'] = codec._V19_SOURCE
        before = {'schema': 'fixture', 'source_cap': codec.SOURCE_CAP,
                  'rows': rows, 'opaque_root': [False, {'keep': None}]}
        target = copy.deepcopy(rows[389]); target['compiled'] = True
        target['current'] = current(source_raw)
        target['views'].append({'name': 'whole-pre-overlay-v2-current', **rows[389]['current']})
        carrier = copy.deepcopy(rows[10]); carrier['fragments'] = [copy.deepcopy(target)]
        predecessor = {'rows': 519, 'logical_sha256': codec.H(codec.canonical(before)), 'extension': {}}
        policy = {'schema': 'semaprax.finite-source-overlay-policy.v2', 'page': {},
                  'predecessor': predecessor, 'replacements': [carrier['source'], target['source']],
                  'additions': [], 'new_fragment_edges': [{'parent': carrier['source'], 'child': target['source']}]}
        logical = {'schema': codec.V8_SCHEMA, 'index': 1, 'total_pages': 1,
                   'predecessor': copy.deepcopy(predecessor), 'replacements': [carrier, target], 'additions': []}
        if indexed:
            logical['replacements'] = [{'old519_index': [index, row['current'], row['views'], row.get('fragments')]} for index, row in zip((10, 389), logical['replacements'])]
        v9 = codec.transform_v9_pin_slots(logical, True); v9['schema'] = codec.V9_SCHEMA
        v10 = codec.encode_v10_with_prefixes(v9, ['fixture/'])
        v11 = codec.transform_v11_digest_slots(v10, True); v11['schema'] = codec.V11_SCHEMA
        inner = codec.encode_v20_compact_inner(codec.encode_v12_view_labels(v11))
        successor = {'source': codec._V19_SOURCE, 'prior_row_sha256': codec.H(codec.canonical(rows[389])),
                     'current': copy.deepcopy(target['current']), 'registration': {
                         'owner': codec._V19_OWNER, 'current': copy.deepcopy(rows[0]['current']),
                         'start': 2, 'text': codec._V19_DECLARATION}}
        wire = codec.encode_v20(inner, successor)
        assert codec.canonical(codec._v20_successor(wire)) == codec.canonical(successor)
        if not compact:wire['compiled_successor'] = copy.deepcopy(successor)
        return before, policy, wire, {codec._V19_OWNER: owner_raw, codec._V19_SOURCE: source_raw}

    def test_whole_original_inverse_and_new_compiled_fragment(self):
        before, policy, wire, _ = self.fixture(); original = codec.canonical(before)
        result = codec.compose_values(before, policy, wire)
        self.assertIs(result['rows'][389]['compiled'], True)
        self.assertEqual(result['rows'][10]['fragments'], [result['rows'][389]])
        self.assertEqual(result['opaque_root'], before['opaque_root'])
        self.assertEqual(result['rows'][389]['opaque'], before['rows'][389]['opaque'])
        self.assertEqual(codec.canonical(before), original)
        with self.assertRaises(AssertionError):codec.compose_values(before, policy, wire['page'])

    def test_singleton_unknown_false_positive_and_stale_witness_refusals(self):
        before, policy, wire, _ = self.fixture()
        mutations = [lambda p: p.update(extra=None),
                     lambda p: p.update(compiled_successor=[p['compiled_successor'], p['compiled_successor']]),
                     lambda p: p['compiled_successor'].update(extra=None),
                     lambda p: p['compiled_successor'].update(source='src/foreign.rs'),
                     lambda p: p['compiled_successor'].update(prior_row_sha256='0'*64),
                     lambda p: p['compiled_successor']['current'].update(sha256='0'*64, archive='e/'+'0'*64),
                     lambda p: p['compiled_successor']['current'].update(archive='arbitrary/normal/path'),
                     lambda p: p['compiled_successor']['registration'].update(extra=None),
                     lambda p: p['compiled_successor']['registration'].update(start=True),
                     lambda p: p['compiled_successor']['registration'].update(text='#[cfg(test)]\n'+codec._V19_DECLARATION),
                     lambda p: p['compiled_successor']['registration']['current'].update(sha256='0'*64, archive='e/'+'0'*64)]
        for mutate in mutations:
            bad = copy.deepcopy(wire); mutate(bad)
            with self.subTest(mutate=mutate), self.assertRaises(AssertionError):codec.compose_values(before, policy, bad)
        for replacements in ([policy['replacements'][0]], policy['replacements'] + [codec._V19_SOURCE]):
            bad_policy = copy.deepcopy(policy);bad_policy['replacements'] = replacements
            with self.assertRaisesRegex(AssertionError, 'v19 explicit replacement'):
                codec.compose_values(before, bad_policy, wire)
        bad_owner = copy.deepcopy(before);bad_owner['rows'][0]['compiled'] = False
        bad_policy = copy.deepcopy(policy);bad_policy['predecessor']['logical_sha256'] = codec.H(codec.canonical(bad_owner))
        bad_wire = copy.deepcopy(wire);bad_wire['page']['predecessor'] = copy.deepcopy(bad_policy['predecessor'])
        with self.assertRaisesRegex(AssertionError, 'v19 owner current'):
            codec.compose_values(bad_owner, bad_policy, bad_wire)
        bad_before = copy.deepcopy(before);bad_before['rows'][389]['compiled'] = True
        bad_policy = copy.deepcopy(policy);bad_policy['predecessor']['logical_sha256'] = codec.H(codec.canonical(bad_before))
        bad_wire = copy.deepcopy(wire);bad_wire['page']['predecessor'] = copy.deepcopy(bad_policy['predecessor'])
        with self.assertRaisesRegex(AssertionError, 'v19 entire old false row'):
            codec.compose_values(bad_before, bad_policy, bad_wire)

    def test_trusted_bindings_precede_every_read_and_physical_drift_refuses(self):
        _, policy, wire, raw = self.fixture(); calls = []
        pins = {s: {'length': len(b), 'sha256': hashlib.sha256(b).hexdigest()} for s, b in raw.items()}
        overlay = codec.Overlay(None, None, None, None, None, None, lambda s: pins[s])
        overlay.load = lambda: (policy, wire)
        def acquire(row):
            calls.append(row['source']); value = raw[row['source']]
            self.assertEqual({'length': len(value), 'sha256': hashlib.sha256(value).hexdigest()},
                             {k: row['current'][k] for k in ('length', 'sha256')})
            return value
        overlay.current = acquire
        self.assertEqual(overlay.load_current(), (policy, wire)); self.assertEqual(calls, [codec._V19_SOURCE, codec._V19_OWNER])
        calls.clear(); pins[codec._V19_SOURCE]['sha256'] = '0'*64
        with self.assertRaisesRegex(AssertionError, 'v19 stale trusted current binding'):overlay.load_current()
        self.assertFalse(calls)
        pins[codec._V19_SOURCE]['sha256'] = wire['compiled_successor']['current']['sha256']
        raw[codec._V19_OWNER] = b'x' + raw[codec._V19_OWNER][1:]
        with self.assertRaises(AssertionError):overlay.load_current()
        raw[codec._V19_OWNER] = b'\nX' + raw[codec._V19_OWNER][2:]
        pins[codec._V19_OWNER]['sha256'] = hashlib.sha256(raw[codec._V19_OWNER]).hexdigest()
        wire['compiled_successor']['registration']['current']['sha256'] = pins[codec._V19_OWNER]['sha256']
        wire['compiled_successor']['registration']['current']['archive'] = 'e/' + pins[codec._V19_OWNER]['sha256']
        with self.assertRaisesRegex(AssertionError, 'v19 declaration attribute boundary'):overlay.load_current()

    def test_compact_tuple_whole_object_inverse_and_hostile_shapes(self):
        fresh = runpy.run_path(str(Path(codec.__file__)), run_name='v19_actual_helper_namespace')
        self.assertIn('re', fresh)
        before, policy, compact, raw = self.fixture(compact=True)
        _, _, original, _ = self.fixture()
        self.assertEqual(fresh['encode_v20'](compact['page'], original['compiled_successor']), compact)
        self.assertEqual(fresh['canonical'](fresh['compose_values'](before, policy, compact)),
                         codec.canonical(codec.compose_values(before, policy, compact)))
        self.assertEqual(codec.canonical(codec._v20_successor(compact)), codec.canonical(original['compiled_successor']))
        self.assertEqual(codec.canonical(codec.compose_values(before, policy, compact)),
                         codec.canonical(codec.compose_values(before, policy, original)))
        self.assertEqual(compact['page'], original['page'])
        malformed = [[], compact['compiled_successor'][:-1], compact['compiled_successor'] + [None]]
        for index, values in [(0, ['other', True, None]), (1, [None, True, '0'*63, 'g'*64]),
                              (2, [True, 1.5, None, -1, codec.SOURCE_CAP+1]),
                              (3, [None, True, '0'*63, 'g'*64]),
                              (4, [True, 1.5, None, -1, codec.SOURCE_CAP+1]),
                              (5, [None, True, '0'*63, 'g'*64]),
                              (6, [True, 1.5, None, -1, codec.SOURCE_CAP+1])]:
            for value in values:
                record = copy.deepcopy(compact['compiled_successor']);record[index] = value;malformed.append(record)
        for record in malformed:
            bad = copy.deepcopy(compact);bad['compiled_successor'] = record
            with self.subTest(record=record), self.assertRaises(AssertionError):codec.compose_values(before, policy, bad)
        pins = {source: {'length': len(value), 'sha256': hashlib.sha256(value).hexdigest()} for source, value in raw.items()}
        calls = [];overlay = codec.Overlay(None, None, None, None, None, None, lambda source: pins[source])
        overlay.current = lambda row: calls.append(row['source']) or raw[row['source']]
        codec.authenticate_v20_registration(overlay, compact)
        self.assertEqual(calls, [codec._V19_SOURCE, codec._V19_OWNER]);calls.clear()
        pins[codec._V19_OWNER]['sha256'] = '0'*64
        with self.assertRaisesRegex(AssertionError, 'v19 stale trusted current binding'):
            codec.authenticate_v20_registration(overlay, compact)
        self.assertFalse(calls)

        indexed_before, indexed_policy, indexed_wire, _ = self.fixture(compact=True, indexed=True)
        indexed_result = codec.compose_values(indexed_before, indexed_policy, indexed_wire)
        old_before, old_policy, old_wire, _ = self.fixture(compact=True)
        self.assertEqual(codec.canonical(indexed_result), codec.canonical(codec.compose_values(old_before, old_policy, old_wire)))
        inner = indexed_wire['page']
        self.assertEqual(inner['schema'], codec.V20_COMPACT_SCHEMA)
        compact_slot = next(i for i, row in enumerate(inner['replacements']) if type(row) is list and type(row[0]) is int)
        for bad_index in (True, 1.0, -1, 519, '0'):
            hostile = copy.deepcopy(indexed_wire); hostile['page']['replacements'][compact_slot][0] = bad_index
            with self.subTest(indexed_reference=bad_index), self.assertRaises(AssertionError):
                codec.compose_values(indexed_before, indexed_policy, hostile)
        hostile = copy.deepcopy(indexed_wire)
        hostile['page']['replacements'][compact_slot] = {'old519_index': hostile['page']['replacements'][compact_slot]}
        with self.assertRaisesRegex(AssertionError, 'v19 indexed wrapped old519 alias'):
            codec.compose_values(indexed_before, indexed_policy, hostile)
        for group in ('additions', 'fragments'):
            hostile = copy.deepcopy(indexed_wire); value = copy.deepcopy(hostile['page']['replacements'][compact_slot])
            if group == 'additions': hostile['page']['additions'].append(value)
            else: hostile['page']['replacements'][compact_slot][3] = [value]
            with self.subTest(indexed_alias=group), self.assertRaises(AssertionError):
                codec.compose_values(indexed_before, indexed_policy, hostile)
        with self.assertRaises(AssertionError): codec.expand_v18_fragment_refs(inner)

    def test_wire_cap_refuses_before_clone(self):
        _, _, wire, _ = self.fixture(); wire['page']['opaque'] = 'x'*codec.PAGE_CAP
        original = codec.copy.deepcopy
        try:
            codec.copy.deepcopy = lambda _: (_ for _ in ()).throw(RuntimeError('cloned before cap'))
            with self.assertRaisesRegex(AssertionError, 'v19 same one-page cap'):codec._v20_successor(wire)
        finally:codec.copy.deepcopy = original

class CompactDictionaryV20(unittest.TestCase):
    @staticmethod
    def current():
        outer=codec.parse((Path(__file__).parent/'page.json').read_bytes())
        if outer['schema']==codec.V20_SCHEMA:
            codec._v20_successor(outer);return outer['page']
        codec._v19_successor(outer)
        logical=codec.expand_v19_indexed_fragment_refs(outer['page'])
        return codec.encode_v20_compact_rows(logical)

    def test_current_whole_inverse_and_dictionary_canonicality(self):
        wire=self.current();logical=codec.expand_v20_compact_fragment_refs(wire)
        self.assertEqual(codec.encode_v20_compact_rows(logical),wire)
        self.assertEqual(len(codec.canonical(logical)),codec._v20_preflight(wire,codec.SOURCE_CAP))
        for decode in (codec.expand_v18_fragment_refs,codec.expand_v19_indexed_fragment_refs):
            with self.assertRaises(AssertionError):decode(wire)
        # Complete legacy fixtures remain on their independent public routes.
        before,policy,old,_=CompiledSuccessorV19.fixture(compact=True,indexed=True)
        entry=codec._v19_successor(old);new=codec.encode_v20(codec.encode_v20_compact_rows(codec.expand_v19_indexed_fragment_refs(old['page'])),entry)
        self.assertEqual(codec.compose_values(before,policy,new),codec.compose_values(before,policy,old))
        self.assertEqual(codec._v20_successor(new),entry)
        prior_fixture=getattr(self,'fixture',None)
        self.fixture=_V20AuthorityCorpus.fixture
        try:
            for name in ('test_whole_original_inverse_and_new_compiled_fragment','test_singleton_unknown_false_positive_and_stale_witness_refusals','test_trusted_bindings_precede_every_read_and_physical_drift_refuses','test_compact_tuple_whole_object_inverse_and_hostile_shapes','test_wire_cap_refuses_before_clone'):
                getattr(_V20AuthorityCorpus,name)(self)
        finally:
            if prior_fixture is None:del self.fixture
            else:self.fixture=prior_fixture

    def test_exact_bounds_precede_clone_and_typed_hostiles_refuse(self):
        wire=self.current();exact=codec._v20_preflight(wire,codec.SOURCE_CAP)
        original=codec.copy.deepcopy
        try:
            codec.copy.deepcopy=lambda _:(_ for _ in ()).throw(RuntimeError('clone before complete bound'))
            with self.assertRaisesRegex(AssertionError,'v18 expanded bound'):codec._v20_preflight(wire,exact-1)
            bad=dict(wire,predecessor='x'*codec.PAGE_CAP)
            with self.assertRaisesRegex(AssertionError,'v20 same one-page cap'):codec.expand_v20_compact_fragment_refs(bad)
        finally:codec.copy.deepcopy=original
        self.assertEqual(codec._v20_preflight(wire,exact),exact)
        at=next(i for i,t in enumerate(wire['pin_tails'])if len(t)==3 and type(t[2])is int)
        for value in (True,False,-1,1.0,len(wire['archive_prefixes'])):
            bad=copy.deepcopy(wire);bad['pin_tails'][at][2]=value
            with self.subTest(archive=value),self.assertRaises(AssertionError):codec.expand_v20_compact_fragment_refs(bad)
        bad=copy.deepcopy(wire);bad['pin_tails'][at][2]=[wire['pin_tails'][at][2],codec._v20_tail_digest(wire,wire['pin_tails'][at])+'.txt']
        with self.assertRaisesRegex(AssertionError,'v20 typed archive literal alias'):codec.expand_v20_compact_fragment_refs(bad)
        for value in ([0,wire['view_labels'][0]],True,None):
            bad=copy.deepcopy(wire);bad['view_labels'][0]=value
            with self.assertRaisesRegex(AssertionError,'v20 sequential labels'):codec.expand_v20_compact_fragment_refs(bad)
        bad=copy.deepcopy(wire);bad['view_labels'].append(bad['view_labels'][0])
        with self.assertRaisesRegex(AssertionError,'v20 sequential labels'):codec.expand_v20_compact_fragment_refs(bad)
        # Alter the actual first two label-reference IDs, retaining the entire
        # dictionary; the first observed ID becomes1 and must be refused.
        bad=copy.deepcopy(wire)
        def views(row):
            if type(row)is list:
                slots=row[2]if type(row[0])is int else row[3]
                fragments=row[3]if type(row[0])is int else(row[4]if len(row)==5 else None)
            else:slots=row.get('views',[]);fragments=row.get('fragments')
            for value in slots:
                if type(value)is list and type(value[0])is int and value[0]in(0,1):value[0]=1-value[0]
            if fragments:
                for child in fragments:
                    if type(child)is list or(type(child)is dict and 'canonical_source'not in child):views(child)
        for group in ('replacements','additions'):
            for row in bad[group]:views(row)
        with self.assertRaisesRegex(AssertionError,'v12 noncanonical first-use order'):codec.expand_v20_compact_fragment_refs(bad)


    def test_unknown_nested_lookalikes_and_complete_dictionary_use_survive(self):
        wire=self.current();at=next(i for i,r in enumerate(wire['replacements'])if type(r)is list and type(r[0])is int)
        opaque={'opaque':{'view_labels':['not a declaration'],'pin_tails':[[0,True,2]],'old519_index':[False]},'unknown':[None,'é\x00']}
        fragments=wire['replacements'][at][3]
        if fragments is None:wire['replacements'][at][3]=[];fragments=wire['replacements'][at][3]
        fragments.append(opaque)
        logical=codec.expand_v20_compact_fragment_refs(wire)
        self.assertEqual(logical['replacements'][at]['old519_index'][3][-1],opaque)
        self.assertEqual(codec.encode_v20_compact_rows(logical),wire)
        bad=copy.deepcopy(wire);bad['view_labels'].append('unused unique declaration')
        with self.assertRaises(AssertionError):codec.expand_v20_compact_fragment_refs(bad)
        bad=copy.deepcopy(wire);bad['pin_tails'].append([0,codec.v11_digest('1'*64,True)])
        with self.assertRaises(AssertionError):codec.expand_v20_compact_fragment_refs(bad)

if __name__ == '__main__':
    unittest.main()
