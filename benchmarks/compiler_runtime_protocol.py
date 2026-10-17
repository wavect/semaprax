"""Closed compatibility boundary for the retained installed app-server protocol.

Schema compatibility is not enforcement authority. This module starts no process.
"""
from __future__ import annotations
import hashlib
import json
import math
from pathlib import Path

PROTOCOL_SHA256 = '1ac6d7c46a319d9c92d4fa78fca78ad01ea2fad82161c8855e837d8e59434077'
MAX_FRAME = 1024 * 1024


class Refusal(ValueError):
    pass


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise Refusal('duplicate JSON field')
        result[key] = value
    return result


def _finite_float(text):
    value = float(text)
    if not math.isfinite(value):
        raise Refusal('nonfinite runtime number')
    return value


def decode(raw: bytes):
    if len(raw) > MAX_FRAME:
        raise Refusal('runtime frame exceeds bound')
    try:
        return json.loads(raw.decode('utf-8'), object_pairs_hook=_pairs, parse_float=_finite_float,
                          parse_constant=lambda _: (_ for _ in ()).throw(Refusal('nonfinite JSON')))
    except (UnicodeError, ValueError, RecursionError) as error:
        raise Refusal('malformed runtime JSON') from error


def encode(value) -> bytes:
    try:
        raw = json.dumps(value, ensure_ascii=False, allow_nan=False,
                         separators=(',', ':')).encode('utf-8') + b'\n'
    except (ValueError, TypeError, RecursionError) as error:
        raise Refusal('unencodable runtime request') from error
    if len(raw) > MAX_FRAME:
        raise Refusal('runtime request exceeds bound')
    return raw


def closed(value, required, optional=()):
    if (type(value) is not dict or not set(required) <= set(value)
            or set(value) - set(required) - set(optional)):
        raise Refusal('runtime object fields differ')
    return value


def _validate(value, schema, document, depth=0):
    if depth > 96:
        raise Refusal('runtime schema depth exceeds bound')
    if schema is True:
        return
    if schema is False or type(schema) is not dict:
        raise Refusal('unsupported runtime schema')
    if '$ref' in schema:
        ref = schema['$ref']
        if not ref.startswith('#/'):
            raise Refusal('foreign runtime schema reference')
        selected = document
        for key in ref[2:].split('/'):
            selected = selected[key.replace('~1', '/').replace('~0', '~')]
        return _validate(value, selected, document, depth + 1)
    for child in schema.get('allOf', []):
        _validate(value, child, document, depth + 1)
    for union in ['oneOf', 'anyOf']:
        if union in schema:
            successes = 0
            common = {k: v for k, v in schema.items() if k != union}
            for child in schema[union]:
                # Installed ServerNotification has a common emittedAtMs field;
                # UserInput variants also carry common fields. They must be
                # validated with the chosen variant, never silently discarded.
                merged = {**common, **child}
                if 'properties' in common or 'properties' in child:
                    merged['properties'] = {**common.get('properties', {}),
                                            **child.get('properties', {})}
                if 'required' in common or 'required' in child:
                    merged['required'] = list(set(common.get('required', [])) |
                                              set(child.get('required', [])))
                try:
                    _validate(value, merged, document, depth + 1)
                    successes += 1
                except Refusal:
                    pass
            if not successes or (union == 'oneOf' and successes != 1):
                raise Refusal('runtime schema variant differs')
            return
    kinds = schema.get('type')
    kinds = kinds if isinstance(kinds, list) else [kinds] if kinds else []
    predicates = {'object': type(value) is dict, 'array': type(value) is list,
                  'string': type(value) is str, 'integer': type(value) is int,
                  'number': type(value) in (int, float), 'boolean': type(value) is bool,
                  'null': value is None}
    if kinds and not any(predicates.get(kind, False) for kind in kinds):
        raise Refusal('runtime value type differs')
    if 'enum' in schema and (value not in schema['enum']
            or not any(type(value) is type(member) for member in schema['enum'])):
        raise Refusal('runtime enum differs')
    if type(value) in (int, float):
        if value < schema.get('minimum', value) or value > schema.get('maximum', value):
            raise Refusal('runtime number outside schema')
    numeric_format = schema.get('format')
    if numeric_format in ['int32', 'int64', 'uint', 'uint16', 'uint32', 'uint64']:
        bits = {'int32': 32, 'int64': 64, 'uint': 64, 'uint16': 16, 'uint32': 32, 'uint64': 64}[numeric_format]
        signed = numeric_format.startswith('int')
        low, high = (-(1 << (bits - 1)), (1 << (bits - 1)) - 1) if signed else (0, (1 << bits) - 1)
        if type(value) is not int or not low <= value <= high:
            raise Refusal('runtime integer format differs')
    if type(value) is str and len(value) < schema.get('minLength', 0):
        raise Refusal('runtime string is too short')
    if type(value) is list:
        if len(value) > 65536 or len(value) < schema.get('minItems', 0):
            raise Refusal('runtime array exceeds bound')
        for item in value:
            _validate(item, schema.get('items', True), document, depth + 1)
    if type(value) is dict and any(key in schema for key in ['type', 'properties', 'required', 'additionalProperties']):
        properties = schema.get('properties', {})
        if not set(schema.get('required', ())) <= set(value):
            raise Refusal('missing runtime field')
        for key, item in value.items():
            # Deliberately stricter than JSON Schema's default open objects:
            # unknown protocol fields require a new reviewed compatibility version.
            child = properties.get(key, schema.get('additionalProperties', False))
            _validate(item, child, document, depth + 1)


class Protocol:
    def __init__(self):
        path = Path(__file__).with_name('compiler_runtime_v1') / 'protocol.json'
        if path.is_symlink():
            raise Refusal('runtime schema must be a regular pinned source')
        raw = path.read_bytes()
        if hashlib.sha256(raw).hexdigest() != PROTOCOL_SHA256:
            raise Refusal('runtime protocol source differs')
        root = decode(raw)
        if root['schema'] != 'semaprax.compiler-runtime-protocol.v1':
            raise Refusal('runtime protocol schema differs')
        self.documents = root['documents']

    def validate(self, name, value):
        if name not in self.documents:
            raise Refusal('unsupported runtime method/schema')
        doc = self.documents[name]
        _validate(value, doc, doc)
        return value
