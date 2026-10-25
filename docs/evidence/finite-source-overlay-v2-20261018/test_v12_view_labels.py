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
_FRONT_PIN = {'length': 1923024, 'sha256': 'f57da8b7b4cb8c8f374bfb05fe6e7e59cb408177f65642e4dfa4aace71de188d'}
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


if __name__ == '__main__':
    unittest.main()
