"""One-invocation memo of pure overlay composition, never physical authority."""
import copy
import json

# Existing overlay SOURCE_CAP: this bounds retained derived data only. It
# neither admits a source nor changes bounded()'s 262144-byte per-read cap.
SOURCE_CAP = 9753094


def encoded(value):
    """Exact JSON types and insertion order; unsupported objects never cache."""
    def closed(item):
        kind = type(item)
        if kind in (str, int, float, bool, type(None)):
            return True
        if kind is list:
            return all(closed(child) for child in item)
        if kind is dict:
            return all(type(key) is str and closed(child) for key, child in item.items())
        return False
    if not closed(value):
        return None
    try:
        return json.dumps(value, ensure_ascii=False, allow_nan=False,
                          separators=(',', ':')).encode('utf8')
    except (ValueError, UnicodeError):
        return None


class DerivedCompose:
    def __init__(self, original):
        self.original = original
        self.key = None
        self.value = None

    def __call__(self, before, policy, page):
        # Inputs have JUST passed original Overlay.load and predecessor reads.
        # Keys own complete bytes, not a digest/pointer/path/stat identity.
        key = tuple(encoded(value) for value in (before, policy, page))
        retainable = all(part is not None for part in key)
        retainable = retainable and sum(map(len, key)) <= SOURCE_CAP
        if retainable and key == self.key:
            return copy.deepcopy(self.value)
        result = self.original(before, policy, page)
        result_bytes = encoded(result) if retainable else None
        if result_bytes is not None and len(result_bytes) <= SOURCE_CAP:
            # Exactly one successful entry; failures never publish partial data.
            self.key, self.value = key, copy.deepcopy(result)
        return copy.deepcopy(result)


class DerivedCompile:
    """Two immutable code objects; every exec still runs in a fresh namespace."""
    PROGRAM_CAP = 65536
    ENTRIES = 2

    def __init__(self, original):
        self.original = original
        self.entries = []

    def __call__(self, *args, **kwargs):
        # The authenticated guard uses only this exact builtin call shape.
        # Other compile modes/options retain their original execution path.
        eligible = not kwargs and len(args) == 3
        eligible = eligible and type(args[0]) in (str, bytes)
        eligible = eligible and type(args[1]) is str and type(args[2]) is str and args[2] == 'exec'
        if eligible:
            try:
                length = len(args[0].encode('utf8')) if type(args[0]) is str else len(args[0])
            except UnicodeError:
                eligible = False
            else:
                eligible = length <= self.PROGRAM_CAP
        if not eligible:
            return self.original(*args, **kwargs)
        key = (type(args[0]), *args)
        for held, code in self.entries:
            if held == key:
                return code
        code = self.original(*args, **kwargs)
        # Compile failures never publish. Exact source, filename and mode are
        # retained, not a digest, stat tuple, path identity or executed globals.
        self.entries.append((key, code))
        del self.entries[:-self.ENTRIES]
        return code


def install(guard):
    """Install only in this freshly authenticated invocation's private namespace."""
    # This closed guard has no inherited future flags. The new callable has
    # the same zero-future compilation context; options bypass its pure memo.
    import __future__
    future_flags = sum(getattr(__future__, name).compiler_flag
                       for name in __future__.all_feature_names)
    assert not guard['_overlay'].__code__.co_flags & future_flags
    guard['compile'] = DerivedCompile(guard.get('compile', compile))
    original_overlay = guard['_overlay']
    memo = None

    def overlay():
        nonlocal memo
        # Executes ALL existing configuration/program/path authentication first;
        # Overlay.compose then executes its unchanged load BEFORE the pure memo.
        owner = original_overlay()
        namespace = guard['_OVERLAY_NS']
        assert namespace['SOURCE_CAP'] == SOURCE_CAP
        if memo is None:
            memo = DerivedCompose(namespace['compose_values'])
            namespace['compose_values'] = memo
        else:
            assert namespace['compose_values'] is memo
        return owner

    guard['_overlay'] = overlay
