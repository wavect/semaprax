#!/usr/bin/env python3
"""Convert pinned rustdoc JSON into the bounded SEMAPRAX index envelope.

This converter never invokes Cargo or rustdoc. Callers must run the explicitly
installed pinned nightly themselves and provide the resulting JSON and its
recorded package, target, feature, and compiler identities.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys
from collections import defaultdict

MAX_INPUT_BYTES = 64 * 1024 * 1024
MAX_INDEX_BYTES = 1_048_576
MAX_ITEMS = 512
MAX_PATH_BYTES = 512
MAX_SIGNATURE_BYTES = 4_096
MAX_TYPE_DEPTH = 32
INDEX_SCHEMA = "semaprax.rust-api-index.v1"
EXTRACTOR_SCHEMA = "semaprax.rustdoc-extractor.v1"


class InputError(Exception):
    pass


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode()


def item_by_id(index: dict, item_id: object) -> dict | None:
    return index.get(str(item_id))


def is_public(item: dict) -> bool:
    return item.get("visibility") == "public"


def item_kind(item: dict) -> str:
    inner = item.get("inner", {})
    if "function" in inner:
        return "function"
    if "assoc_type" in inner:
        return "associated_type"
    if "trait" in inner:
        return "trait"
    if "struct" in inner:
        return "struct"
    if "enum" in inner:
        return "enum"
    if "union" in inner:
        return "union"
    if "module" in inner:
        return "module"
    if "use" in inner:
        return "use"
    if "impl" in inner:
        return "impl"
    return next(iter(inner), "unknown")


def module_exports(document: dict, crate_name: str) -> dict[str, set[tuple[str, ...]]]:
    """Resolve public module children and use/glob re-exports to public paths."""
    index = document["index"]
    exports: dict[str, set[tuple[str, ...]]] = defaultdict(set)
    root_id = str(document["root"])
    root = index.get(root_id)
    if not root or "module" not in root.get("inner", {}):
        raise InputError("rustdoc JSON root is not a module")
    root_path = (crate_name,)
    seen_modules: set[tuple[str, tuple[str, ...]]] = set()

    def add_item(item_id: object, path: tuple[str, ...]) -> None:
        stable_id = str(item_id)
        item = index.get(stable_id)
        if item is None or len("::".join(path).encode()) > MAX_PATH_BYTES:
            return
        exports[stable_id].add(path)

    def visit_contents(module_id: object, prefix: tuple[str, ...], stack: frozenset[str]) -> None:
        module_key = str(module_id)
        if module_key in stack or (module_key, prefix) in seen_modules:
            return
        seen_modules.add((module_key, prefix))
        module = index.get(module_key)
        if not module or "module" not in module.get("inner", {}):
            return
        children = module["inner"]["module"].get("items", [])
        for child_id in children:
            child = item_by_id(index, child_id)
            if not child:
                continue
            child_kind = item_kind(child)
            if child_kind == "use":
                use = child["inner"]["use"]
                if not is_public(child):
                    continue
                target_id = use.get("id")
                name = use.get("name")
                if use.get("is_glob"):
                    visit_glob(target_id, prefix, stack | {module_key})
                elif name and target_id is not None:
                    alias_path = prefix + (name,)
                    add_item(target_id, alias_path)
                    target = index.get(str(target_id), {})
                    if "module" in target.get("inner", {}):
                        visit_contents(target_id, alias_path, stack | {module_key})
                continue
            name = child.get("name")
            if not name or not is_public(child):
                continue
            child_path = prefix + (name,)
            add_item(child_id, child_path)
            if child_kind == "module":
                visit_contents(child_id, child_path, stack | {module_key})

    def visit_glob(module_id: object, prefix: tuple[str, ...], stack: frozenset[str]) -> None:
        module_key = str(module_id)
        if module_key in stack:
            return
        module = index.get(module_key)
        if not module or "module" not in module.get("inner", {}):
            return
        for child_id in module["inner"]["module"].get("items", []):
            child = item_by_id(index, child_id)
            if not child:
                continue
            if item_kind(child) == "use":
                use = child["inner"]["use"]
                if not is_public(child):
                    continue
                target_id = use.get("id")
                name = use.get("name")
                if use.get("is_glob"):
                    visit_glob(target_id, prefix, stack | {module_key})
                elif name and target_id is not None:
                    add_item(target_id, prefix + (name,))
                continue
            name = child.get("name")
            if not name or not is_public(child):
                continue
            child_path = prefix + (name,)
            add_item(child_id, child_path)
            if item_kind(child) == "module":
                visit_contents(child_id, child_path, stack | {module_key})

    visit_contents(root_id, root_path, frozenset())

    return exports


def format_type(node: object, document: dict, exports: dict, depth: int = 1) -> tuple[str, int, bool, bool]:
    """Return spelling, depth, representable, and opaque-return facts."""
    if depth > MAX_TYPE_DEPTH or not isinstance(node, dict) or len(node) != 1:
        return "?", min(depth, MAX_TYPE_DEPTH), False, False
    tag, value = next(iter(node.items()))
    if tag == "primitive" and isinstance(value, str):
        return value, depth, True, False
    if tag == "generic" and isinstance(value, str):
        return value, depth, False, False
    if tag == "infer":
        return "_", depth, False, False
    if tag == "borrowed_ref" and isinstance(value, dict):
        inner, child_depth, ok, opaque = format_type(value.get("type"), document, exports, depth + 1)
        lifetime = value.get("lifetime")
        prefix = "&" + (f"{lifetime} " if lifetime else "")
        mutable = "mut " if value.get("is_mutable") else ""
        return f"{prefix}{mutable}{inner}", child_depth, ok, opaque
    if tag == "raw_pointer" and isinstance(value, dict):
        inner, child_depth, ok, opaque = format_type(value.get("type"), document, exports, depth + 1)
        return f"*{'mut' if value.get('is_mutable') else 'const'} {inner}", child_depth, False, opaque
    if tag == "slice":
        inner, child_depth, ok, opaque = format_type(value, document, exports, depth + 1)
        return f"[{inner}]", child_depth, ok, opaque
    if tag == "array" and isinstance(value, dict):
        inner, child_depth, ok, opaque = format_type(value.get("type"), document, exports, depth + 1)
        return f"[{inner}; {value.get('len', '?')} ]", child_depth, False, opaque
    if tag == "tuple" and isinstance(value, list):
        parts = [format_type(entry, document, exports, depth + 1) for entry in value]
        return "(" + ", ".join(part[0] for part in parts) + ")", max([depth, *(p[1] for p in parts)]), all(p[2] for p in parts), any(p[3] for p in parts)
    if tag == "resolved_path" and isinstance(value, dict):
        path_id = str(value.get("id", ""))
        item_paths = exports.get(path_id, set())
        if item_paths:
            path = "::".join(min(item_paths, key=lambda p: (len(p), p)))
        else:
            summary = document.get("paths", {}).get(path_id, {})
            path_parts = summary.get("path")
            path = "::".join(path_parts) if isinstance(path_parts, list) and path_parts else str(value.get("path", "?"))
        args_node = value.get("args")
        args: list[str] = []
        child_depths = [depth]
        representable = bool(path and "?" not in path)
        opaque = False
        if isinstance(args_node, dict):
            angle = args_node.get("angle_bracketed", {})
            for arg in angle.get("args", []):
                if "type" in arg:
                    part, child_depth, ok, is_opaque = format_type(arg["type"], document, exports, depth + 1)
                    args.append(part)
                    child_depths.append(child_depth)
                    representable &= ok
                    opaque |= is_opaque
                elif "lifetime" in arg:
                    args.append(str(arg["lifetime"]))
                elif "const" in arg:
                    args.append(str(arg["const"]))
                    representable = False
                else:
                    representable = False
            if angle.get("constraints"):
                representable = False
        if args:
            path += "<" + ", ".join(args) + ">"
        return path, max(child_depths), representable, opaque
    if tag == "qualified_path" and isinstance(value, dict):
        base, child_depth, ok, opaque = format_type(value.get("self_type"), document, exports, depth + 1)
        trait = value.get("trait") or {}
        trait_path = str(trait.get("path", ""))
        return f"<{base} as {trait_path}>::{value.get('name', '?')}", child_depth, False and ok, opaque
    if tag in ("impl_trait", "dyn_trait"):
        return "impl _", depth + 1, False, tag == "impl_trait"
    return "?", depth, False, False


def receiver_for(inputs: list) -> tuple[str, list]:
    if not inputs or inputs[0][0] != "self":
        return "none", inputs
    typ = inputs[0][1]
    if "borrowed_ref" in typ:
        return ("mutable" if typ["borrowed_ref"].get("is_mutable") else "shared"), inputs[1:]
    if "generic" in typ:
        return "owned", inputs[1:]
    return "owned", inputs[1:]


def function_record(path: tuple[str, ...], item: dict, kind: str, document: dict, exports: dict) -> dict:
    function = item.get("inner", {}).get("function", {})
    signature = function.get("sig", {})
    inputs = signature.get("inputs", [])
    receiver, arguments = receiver_for(inputs)
    rendered_args: list[str] = []
    max_depth = 1
    supported = not signature.get("is_c_variadic", False)
    opaque = False
    for name, typ in arguments:
        rendered, depth, representable, has_opaque = format_type(typ, document, exports)
        rendered_args.append(f"{name}: {rendered}")
        max_depth = max(max_depth, depth)
        supported &= representable
        opaque |= has_opaque
    output_type = signature.get("output")
    if output_type is None:
        rendered_output = "()"
    else:
        rendered_output, depth, representable, has_opaque = format_type(output_type, document, exports)
        max_depth = max(max_depth, depth)
        supported &= representable
        opaque |= has_opaque
    receiver_arg = {
        "shared": "&self",
        "mutable": "&mut self",
        "owned": "self",
        "none": None,
    }[receiver]
    if receiver_arg:
        rendered_args.insert(0, receiver_arg)
    params = function.get("generics", {}).get("params", [])
    unsupported_generics = any(
        "lifetime" not in parameter.get("kind", {})
        or parameter.get("kind", {}).get("lifetime", {}).get("outlives")
        for parameter in params
    ) or bool(function.get("generics", {}).get("where_predicates"))
    if unsupported_generics:
        supported = False
    name = item.get("name") or path[-1]
    generic_text = ""
    if params:
        generic_text = "<" + ", ".join(str(p.get("name", "_")) for p in params) + ">"
    rendered_signature = f"fn {name}{generic_text}({', '.join(rendered_args)}) -> {rendered_output}"
    if len(rendered_signature.encode()) > MAX_SIGNATURE_BYTES:
        supported = False
        rendered_signature = rendered_signature.encode()[:MAX_SIGNATURE_BYTES].decode("utf-8", "ignore")
    if max_depth > MAX_TYPE_DEPTH:
        max_depth = MAX_TYPE_DEPTH
        supported = False
        reason = "expansion_limit"
    elif opaque:
        reason = "opaque_return"
        supported = False
    elif unsupported_generics:
        reason = "unsupported_generic"
        supported = False
    elif not supported:
        reason = "unsupported_signature"
    else:
        reason = None
    return {
        "path": "::".join(path),
        "kind": kind,
        "receiver": receiver,
        "signature": rendered_signature,
        "type_depth": max_depth,
        "support": "supported" if supported else "rejected",
        "reason": reason,
    }


def extract(document: dict, args: argparse.Namespace, extractor_digest: str) -> dict:
    if document.get("format_version") != args.rustdoc_format_version:
        raise InputError("rustdoc JSON format version does not match the pinned extractor argument")
    target = document.get("target", {}).get("triple")
    if target != args.target:
        raise InputError("rustdoc JSON target does not match the selected target")
    index = document.get("index")
    if not isinstance(index, dict):
        raise InputError("rustdoc JSON item index is missing")
    exports = module_exports(document, args.package_name)
    records: list[dict] = []
    trait_owner: dict[str, str] = {}
    inherent_owner: dict[str, str] = {}
    for owner_id, item in index.items():
        inner = item.get("inner", {})
        if "trait" in inner:
            for member in inner["trait"].get("items", []):
                trait_owner[str(member)] = str(owner_id)
        if "impl" in inner and inner["impl"].get("trait") is None:
            owner_type = inner["impl"].get("for", {}).get("resolved_path", {}).get("id")
            if owner_type is None:
                continue
            for member in inner["impl"].get("items", []):
                inherent_owner[str(member)] = str(owner_type)

    for item_id, item in index.items():
        inner = item.get("inner", {})
        kind = item_kind(item)
        if kind == "function":
            owner_id = trait_owner.get(str(item_id)) or inherent_owner.get(str(item_id))
            if owner_id:
                owner = index.get(owner_id, {})
                if str(item_id) in trait_owner and not is_public(owner):
                    continue
                if str(item_id) in inherent_owner and not is_public(item):
                    continue
                owner_paths = exports.get(owner_id, set())
                if not owner_paths:
                    continue
                method_kind = "trait_method" if str(item_id) in trait_owner else "inherent_method"
                for owner_path in owner_paths:
                    records.append(function_record(owner_path + (item.get("name", "?"),), item, method_kind, document, exports))
            elif is_public(item):
                for path in exports.get(str(item_id), set()):
                    records.append(function_record(path, item, "function", document, exports))
        elif kind == "assoc_type":
            owner_id = trait_owner.get(str(item_id))
            owner = index.get(owner_id or "", {})
            if owner_id and is_public(owner):
                for owner_path in exports.get(owner_id, set()):
                    records.append({
                        "path": "::".join(owner_path + (item.get("name", "?"),)),
                        "kind": "associated_type",
                        "receiver": "none",
                        "signature": f"type {item.get('name', '?')}",
                        "type_depth": 1,
                        "support": "rejected",
                        "reason": "unsupported_signature",
                    })

    # Keep deterministic unique public paths. If rustdoc finds multiple
    # definitions for one path, ambiguity is a hard extractor failure.
    records.sort(key=lambda record: record["path"].encode())
    unique: list[dict] = []
    previous = None
    for record in records:
        path = record["path"]
        if previous == path:
            raise InputError(f"rustdoc JSON produced duplicate public path {path}")
        previous = path
        if len(path.encode()) > MAX_PATH_BYTES:
            raise InputError("public API path exceeds its byte limit")
        unique.append(record)
    if args.select:
        selected = set(args.select)
        unique = [row for row in unique if row["path"] in selected]
        found = {row["path"] for row in unique}
        missing = sorted(selected - found)
        if missing:
            raise InputError("selected paths were not public items in this target/features JSON: " + ", ".join(missing))
    if len(unique) > MAX_ITEMS:
        raise InputError(f"public API inventory has {len(unique)} rows (limit {MAX_ITEMS}); select a bounded API closure")
    package = {
        "name": args.package_name,
        "version": args.package_version,
        "source_sha256": args.source_sha256,
        "renamed_from": args.renamed_from,
    }
    index_doc = {
        "schema": INDEX_SCHEMA,
        "package": package,
        "target": args.target,
        "feature_digest": args.feature_digest,
        "extractor": {
            "mode": "nightly-rustdoc-json",
            "executable_sha256": extractor_digest,
            "rustc_version": args.rustdoc_version,
            "rustdoc_format": f"rustdoc-json:{args.rustdoc_format_version}",
        },
        "items": unique,
        "limits": {
            "max_items": MAX_ITEMS,
            "max_type_depth": MAX_TYPE_DEPTH,
            "max_index_bytes": MAX_INDEX_BYTES,
        },
    }
    envelope = {"schema": EXTRACTOR_SCHEMA, "index": index_doc}
    output = canonical_json(envelope)
    # Estimate the canonical inner index too; final admission repeats this
    # exact bound using the SEMAPRAX canonical renderer.
    if len(canonical_json(index_doc)) > MAX_INDEX_BYTES:
        raise InputError("canonical Rust API index exceeds the 1 MiB limit; select fewer paths")
    return envelope


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rustdoc-json", required=True, type=pathlib.Path)
    parser.add_argument("--package-name", required=True)
    parser.add_argument("--package-version", required=True)
    parser.add_argument("--source-sha256", required=True)
    parser.add_argument("--renamed-from")
    parser.add_argument("--target", required=True)
    parser.add_argument("--feature-digest", required=True)
    parser.add_argument("--rustdoc-version", required=True)
    parser.add_argument("--rustdoc-format-version", required=True, type=int)
    parser.add_argument("--select", action="append", default=[])
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    try:
        if not args.rustdoc_version.startswith("rustdoc ") or "nightly" not in args.rustdoc_version:
            raise InputError("extractor mode requires the explicitly pinned nightly rustdoc version")
        for digest in (args.source_sha256, args.feature_digest):
            if len(digest) != 71 or not digest.startswith("sha256:") or any(c not in "0123456789abcdef" for c in digest[7:]):
                raise InputError("SHA-256 values must use sha256: and lowercase hexadecimal")
        raw = args.rustdoc_json.read_bytes()
        if not raw or len(raw) > MAX_INPUT_BYTES:
            raise InputError("rustdoc JSON input is empty or exceeds the 64 MiB extractor bound")
        document = json.loads(raw)
        extractor_digest = "sha256:" + hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()
        output = canonical_json(extract(document, args, extractor_digest))
        if len(output) > MAX_INDEX_BYTES:
            raise InputError("extractor envelope exceeds the 1 MiB replay bound")
        if args.output:
            args.output.write_bytes(output)
        else:
            sys.stdout.buffer.write(output)
    except (OSError, UnicodeError, json.JSONDecodeError, InputError) as error:
        print(f"rustdoc-json-to-index: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
