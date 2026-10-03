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
MAX_TYPES = 512
MAX_PATH_BYTES = 512
MAX_SIGNATURE_BYTES = 4_096
MAX_DOC_BYTES = 16_384
MAX_TOTAL_DOC_BYTES = 524_288
MAX_GENERIC_PARAMS = 64
MAX_GENERIC_METADATA_BYTES = 16_384
MAX_TYPE_REFERENCES = 256
MAX_TYPE_DEPTH = 32
INDEX_SCHEMA = "semaprax.rust-api-index.v2"
EXTRACTOR_SCHEMA = "semaprax.rustdoc-extractor.v2"


class InputError(Exception):
    pass


def canonical_json(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode()


def item_by_id(index: dict, item_id: object) -> dict | None:
    return index.get(str(item_id))


def is_public(item: dict) -> bool:
    return item.get("visibility") == "public"


def visibility_name(item: dict, inherited_public: bool = False) -> str:
    visibility = item.get("visibility")
    if visibility == "public" or (visibility == "default" and inherited_public):
        return "public"
    if visibility == "default":
        return "private"
    if visibility == "crate":
        return "crate"
    if isinstance(visibility, dict) and set(visibility) == {"restricted"}:
        return "restricted"
    raise InputError(f"unsupported or missing Rust visibility: {visibility!r}")


def docs_text(item: dict) -> str | None:
    docs = item.get("docs")
    if docs is None:
        return None
    if not isinstance(docs, str) or "\0" in docs or len(docs.encode()) > MAX_DOC_BYTES:
        raise InputError("item documentation is malformed or exceeds the per-item bound")
    return docs


def source_span(item: dict, args: argparse.Namespace) -> dict | None:
    span = item.get("span")
    if span is None:
        return None
    if not isinstance(span, dict):
        raise InputError("rustdoc span is malformed")
    filename = span.get("filename")
    begin = span.get("begin")
    end = span.get("end")
    if not isinstance(filename, str) or not isinstance(begin, list) or not isinstance(end, list):
        raise InputError("rustdoc span is incomplete")
    root = args.source_root.resolve()
    source = pathlib.Path(filename)
    if source.is_absolute():
        try:
            source = source.resolve().relative_to(root)
        except ValueError as error:
            raise InputError("rustdoc span is outside the declared package source root") from error
    else:
        source = pathlib.Path(*pathlib.PurePosixPath(filename.replace("\\", "/")).parts)
        if source.is_absolute() or any(part in ("", ".", "..") for part in source.parts):
            raise InputError("rustdoc span path is not normalized")
    if len(begin) != 2 or len(end) != 2 or any(not isinstance(n, int) for n in begin + end):
        raise InputError("rustdoc span coordinates are malformed")
    start_line, start_column = begin
    end_line, end_column = end
    if start_line < 1 or start_column < 1 or end_line < start_line or (end_line == start_line and end_column < start_column):
        raise InputError("rustdoc span coordinates are out of order")
    path = source.as_posix()
    if not path or len(path.encode()) > MAX_PATH_BYTES:
        raise InputError("normalized rustdoc span path exceeds its bound")
    return {"file": path, "start_line": start_line, "start_column": start_column, "end_line": end_line, "end_column": end_column}


def json_fragment(value: object) -> str:
    text = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    if len(text.encode()) > MAX_GENERIC_METADATA_BYTES:
        raise InputError("generic bound metadata exceeds its per-fragment bound")
    return text


def generic_metadata(generics: dict, document: dict, exports: dict, references: set[str]) -> dict:
    params = generics.get("params", [])
    where_predicates = generics.get("where_predicates", [])
    if not isinstance(params, list) or len(params) > MAX_GENERIC_PARAMS or not isinstance(where_predicates, list) or len(where_predicates) > MAX_GENERIC_PARAMS:
        raise InputError("generic parameter or where-predicate count exceeds its bound")
    rendered = []
    for parameter in params:
        name = parameter.get("name")
        kind = parameter.get("kind", {})
        if not isinstance(name, str) or len(name.encode()) > MAX_PATH_BYTES or len(kind) != 1:
            raise InputError("generic parameter is malformed")
        tag, details = next(iter(kind.items()))
        if tag not in ("type", "lifetime", "const") or not isinstance(details, dict):
            raise InputError("generic parameter kind is unsupported")
        if tag == "type":
            bounds = details.get("bounds", [])
            default_node = details.get("default")
            default = None if default_node is None else format_type(default_node, document, exports, references=references)[0]
            const_type = None
        elif tag == "lifetime":
            bounds = [{"outlives": details.get("outlives", [])}]
            default = None
            const_type = None
        else:
            bounds = []
            const_type = format_type(details.get("type"), document, exports, references=references)[0]
            default = None if details.get("default") is None else str(details["default"])
        if not isinstance(bounds, list) or len(bounds) > MAX_GENERIC_PARAMS:
            raise InputError("generic bound count exceeds its bound")
        for bound in bounds:
            collect_type_ids(bound, references)
        rendered.append({
            "name": name,
            "kind": tag,
            "bounds": [json_fragment(bound) for bound in bounds],
            "default": default,
            "const_type": const_type,
        })
    for predicate in where_predicates:
        collect_type_ids(predicate, references)
    return {
        "parameters": rendered,
        "where_predicates": [json_fragment(predicate) for predicate in where_predicates],
    }


def collect_type_ids(value: object, references: set[str]) -> None:
    if isinstance(value, dict):
        if "id" in value and "path" in value:
            references.add(str(value["id"]))
        for child in value.values():
            collect_type_ids(child, references)
    elif isinstance(value, list):
        for child in value:
            collect_type_ids(child, references)


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


def format_type(node: object, document: dict, exports: dict, depth: int = 1, references: set[str] | None = None) -> tuple[str, int, bool, bool]:
    """Return spelling, depth, representable, and opaque-return facts."""
    if references is not None:
        collect_type_ids(node, references)
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
        inner, child_depth, ok, opaque = format_type(value.get("type"), document, exports, depth + 1, references)
        lifetime = value.get("lifetime")
        prefix = "&" + (f"{lifetime} " if lifetime else "")
        mutable = "mut " if value.get("is_mutable") else ""
        return f"{prefix}{mutable}{inner}", child_depth, ok, opaque
    if tag == "raw_pointer" and isinstance(value, dict):
        inner, child_depth, ok, opaque = format_type(value.get("type"), document, exports, depth + 1, references)
        return f"*{'mut' if value.get('is_mutable') else 'const'} {inner}", child_depth, False, opaque
    if tag == "slice":
        inner, child_depth, ok, opaque = format_type(value, document, exports, depth + 1, references)
        return f"[{inner}]", child_depth, ok, opaque
    if tag == "array" and isinstance(value, dict):
        inner, child_depth, ok, opaque = format_type(value.get("type"), document, exports, depth + 1, references)
        return f"[{inner}; {value.get('len', '?')} ]", child_depth, False, opaque
    if tag == "tuple" and isinstance(value, list):
        parts = [format_type(entry, document, exports, depth + 1, references) for entry in value]
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
                    part, child_depth, ok, is_opaque = format_type(arg["type"], document, exports, depth + 1, references)
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
        base, child_depth, ok, opaque = format_type(value.get("self_type"), document, exports, depth + 1, references)
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


def function_record(
    path: tuple[str, ...], item: dict, kind: str, document: dict, exports: dict,
    args: argparse.Namespace, owner_id: str | None = None,
    inherited_public: bool = False, sealed: bool = False,
) -> dict:
    function = item.get("inner", {}).get("function", {})
    signature = function.get("sig", {})
    inputs = signature.get("inputs", [])
    receiver, arguments = receiver_for(inputs)
    references: set[str] = set()
    rendered_args: list[str] = []
    max_depth = 1
    supported = not signature.get("is_c_variadic", False)
    opaque = False
    for name, typ in arguments:
        rendered, depth, representable, has_opaque = format_type(typ, document, exports, references=references)
        rendered_args.append(f"{name}: {rendered}")
        max_depth = max(max_depth, depth)
        supported &= representable
        opaque |= has_opaque
    output_type = signature.get("output")
    if output_type is None:
        rendered_output = "()"
    else:
        rendered_output, depth, representable, has_opaque = format_type(output_type, document, exports, references=references)
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
    generics = generic_metadata(function.get("generics", {}), document, exports, references)
    if owner_id is not None:
        references.add(str(owner_id))
    params = generics["parameters"]
    unsupported_generics = any(parameter["kind"] != "lifetime" for parameter in params) or bool(generics["where_predicates"])
    if unsupported_generics:
        supported = False
    name = item.get("name") or path[-1]
    generic_text = ""
    if params:
        generic_text = "<" + ", ".join(parameter["name"] for parameter in params) + ">"
    rendered_signature = f"fn {name}{generic_text}({', '.join(rendered_args)}) -> {rendered_output}"
    if len(rendered_signature.encode()) > MAX_SIGNATURE_BYTES:
        supported = False
        rendered_signature = rendered_signature.encode()[:MAX_SIGNATURE_BYTES].decode("utf-8", "ignore")
    visibility = visibility_name(item, inherited_public=inherited_public)
    if visibility != "public":
        reason = "private"
        supported = False
    elif sealed:
        reason = "sealed_trait"
        supported = False
    elif max_depth > MAX_TYPE_DEPTH:
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
        "visibility": visibility,
        "docs": docs_text(item),
        "span": source_span(item, args),
        "generics": generics,
        "associated_type": None,
        "_type_root_ids": sorted(references),
        "support": "supported" if supported else "rejected",
        "reason": reason,
    }


def associated_type_record(
    path: tuple[str, ...], item: dict, document: dict, exports: dict, args: argparse.Namespace,
    owner_id: str, sealed: bool,
) -> dict:
    associated = item.get("inner", {}).get("assoc_type", {})
    references: set[str] = set()
    bounds = associated.get("bounds", [])
    for bound in bounds:
        collect_type_ids(bound, references)
    raw_default = associated.get("type")
    default = None
    if raw_default is not None:
        default = format_type(raw_default, document, exports, references=references)[0]
    references.add(str(owner_id))
    generics = generic_metadata(associated.get("generics", {}), document, exports, references)
    associated_metadata = {"bounds": [json_fragment(bound) for bound in bounds], "default": default}
    visibility = visibility_name(item, inherited_public=True)
    return {
        "path": "::".join(path),
        "kind": "associated_type",
        "receiver": "none",
        "signature": f"type {item.get('name', path[-1])}",
        "type_depth": 1,
        "visibility": visibility,
        "docs": docs_text(item),
        "span": source_span(item, args),
        "generics": generics,
        "associated_type": associated_metadata,
        "_type_root_ids": sorted(references),
        "support": "rejected",
        "reason": "private" if visibility != "public" else "sealed_trait" if sealed else "unsupported_signature",
    }


def trait_is_sealed(item: dict, index: dict, exports: dict) -> bool:
    trait = item.get("inner", {}).get("trait", {})
    for bound in trait.get("bounds", []):
        target = bound.get("trait_bound", {}).get("trait", {})
        target_item = index.get(str(target.get("id", "")))
        if target_item is not None and str(target.get("id")) not in exports:
            return True
    return False


def path_for_id(item_id: str, document: dict, exports: dict, trait_owner: dict[str, str]) -> str:
    item_paths = exports.get(item_id, set())
    if item_paths:
        path = "::".join(min(item_paths, key=lambda value: (len(value), value)))
    else:
        index = document["index"]
        owner_id = trait_owner.get(item_id)
        summary = document.get("paths", {}).get(item_id, {})
        path_parts = summary.get("path")
        if isinstance(path_parts, list) and path_parts:
            path = "::".join(path_parts)
        elif owner_id and item_id in index:
            owner_path = path_for_id(owner_id, document, exports, trait_owner)
            name = index[item_id].get("name")
            if not isinstance(name, str):
                raise InputError(f"rustdoc associated item {item_id} has no name")
            path = owner_path + "::" + name
        elif item_id in index:
            name = index[item_id].get("name", "item")
            path = f"__rustdoc_private::item_{item_id}::{name}"
        else:
            raise InputError(f"rustdoc did not resolve a reachable type path for item {item_id}")
    if not path or len(path.encode()) > MAX_PATH_BYTES:
        raise InputError("reachable type path is empty or exceeds its bound")
    return path


def field_type_ids(field_ids: object, index: dict, references: set[str]) -> None:
    if not isinstance(field_ids, list):
        raise InputError("rustdoc type field list is malformed")
    for field_id in field_ids:
        if field_id is None:
            continue
        field = index.get(str(field_id))
        if field is None or "struct_field" not in field.get("inner", {}):
            raise InputError("rustdoc field reference is missing")
        if field.get("visibility") == "public":
            collect_type_ids(field["inner"]["struct_field"], references)


def type_record_for_id(
    item_id: str, document: dict, exports: dict, trait_owner: dict[str, str], args: argparse.Namespace,
) -> tuple[dict, set[str]]:
    index = document["index"]
    item = index.get(item_id)
    summary = document.get("paths", {}).get(item_id, {})
    references: set[str] = set()
    if item is None:
        if not summary:
            raise InputError(f"rustdoc type closure has an unresolved ID {item_id}")
        path = path_for_id(item_id, document, exports, trait_owner)
        return ({
            "path": path,
            "kind": "external",
            "visibility": "external",
            "docs": None,
            "span": None,
            "generics": {"parameters": [], "where_predicates": []},
            "references": [],
        }, references)

    owner_id = trait_owner.get(item_id)
    owner = index.get(owner_id or "", {})
    kind = item_kind(item)
    container_kind = "assoc_type" if kind == "associated_type" else kind
    if kind == "associated_type":
        visibility = visibility_name(item, inherited_public=is_public(owner) and bool(exports.get(owner_id or "")))
    else:
        visibility = visibility_name(item)
        if visibility == "public" and item_id not in exports:
            visibility = "private"
    inner = item.get("inner", {})
    container = inner.get(container_kind, {})
    if kind == "struct":
        shape = container.get("kind")
        if isinstance(shape, dict):
            if "plain" in shape:
                field_type_ids(shape["plain"].get("fields", []), index, references)
            elif "tuple" in shape:
                field_type_ids(shape["tuple"], index, references)
    elif kind == "enum":
        for variant_id in container.get("variants", []):
            variant = index.get(str(variant_id), {})
            variant_data = variant.get("inner", {}).get("enum_variant", {})
            field_type_ids(variant_data.get("fields", []), index, references)
    elif kind == "union":
        field_type_ids(container.get("fields", []), index, references)
    elif kind == "type_alias":
        collect_type_ids(container.get("type"), references)
    elif kind == "trait":
        collect_type_ids(container.get("bounds", []), references)
        references.update(
            str(member_id) for member_id in container.get("items", [])
            if item_kind(index.get(str(member_id), {})) == "associated_type"
        )
    elif kind == "associated_type":
        associated = inner.get("assoc_type", {})
        collect_type_ids(associated.get("bounds", []), references)
        collect_type_ids(associated.get("type"), references)
    elif kind == "function":
        function = inner.get("function", {})
        signature = function.get("sig", {})
        collect_type_ids(signature.get("inputs", []), references)
        collect_type_ids(signature.get("output"), references)
    else:
        kind = "other"

    generic_source = container.get("generics", {}) if isinstance(container, dict) else {}
    generics = generic_metadata(generic_source, document, exports, references)
    if kind == "associated_type":
        generics = generic_metadata(inner.get("assoc_type", {}).get("generics", {}), document, exports, references)
    record = {
        "path": path_for_id(item_id, document, exports, trait_owner),
        "kind": {"struct": "struct", "enum": "enum", "union": "union", "type_alias": "type_alias", "trait": "trait", "associated_type": "associated_type"}.get(kind, "other"),
        "visibility": visibility,
        "docs": docs_text(item),
        "span": source_span(item, args),
        "generics": generics,
        "references": [],
    }
    return record, references


def build_type_closures(
    records: list[dict], document: dict, exports: dict, trait_owner: dict[str, str], args: argparse.Namespace,
) -> list[dict]:
    types_by_path: dict[str, dict] = {}
    type_nodes_by_id: dict[str, tuple[dict, set[str]]] = {}
    for record in records:
        root_ids = record.pop("_type_root_ids", [])
        root_paths = sorted({path_for_id(str(item_id), document, exports, trait_owner) for item_id in root_ids})
        record["type_roots"] = root_paths
        pending = [(str(item_id), 1) for item_id in root_ids]
        reached: dict[str, int] = {}
        while pending:
            item_id, depth = pending.pop(0)
            if item_id in reached:
                continue
            if depth > MAX_TYPE_DEPTH:
                raise InputError("reachable type closure exceeds the configured depth; select fewer API paths")
            if len(reached) >= MAX_TYPE_REFERENCES:
                raise InputError("reachable type closure exceeds its per-item bound; select fewer API paths")
            reached[item_id] = depth
            if item_id in type_nodes_by_id:
                node, child_ids = type_nodes_by_id[item_id]
            else:
                node, child_ids = type_record_for_id(item_id, document, exports, trait_owner, args)
                child_paths = sorted(
                    {path_for_id(child, document, exports, trait_owner) for child in child_ids},
                    key=lambda value: value.encode(),
                )
                node["references"] = child_paths
                type_nodes_by_id[item_id] = (node, child_ids)
            path = node["path"]
            existing = types_by_path.get(path)
            if existing is not None and existing != node:
                raise InputError(f"distinct rustdoc types collide at public path {path}")
            types_by_path[path] = node
            children = sorted({str(child) for child in child_ids}, key=lambda value: path_for_id(value, document, exports, trait_owner).encode())
            for child in children:
                if child not in reached:
                    pending.append((child, depth + 1))
        reachable = sorted({path_for_id(item_id, document, exports, trait_owner) for item_id in reached})
        record["reachable_types"] = reachable
        record["type_closure_depth"] = max(reached.values(), default=0)
        record["closure_complete"] = all(
            types_by_path[path]["visibility"] == "public"
            and types_by_path[path]["kind"] not in ("external", "other")
            for path in reachable
        )
        if not record["closure_complete"] and record["support"] == "supported":
            record["support"] = "rejected"
            record["reason"] = "incomplete_type_closure"
    if len(types_by_path) > MAX_TYPES:
        raise InputError("selected APIs reach too many types; select fewer API paths")
    return [types_by_path[path] for path in sorted(types_by_path, key=lambda value: value.encode())]


def extract(document: dict, args: argparse.Namespace, extractor_digest: str) -> dict:
    if document.get("format_version") != args.rustdoc_format_version:
        raise InputError("rustdoc JSON format version does not match the pinned extractor argument")
    target = document.get("target", {}).get("triple")
    if target != args.target:
        raise InputError("rustdoc JSON target does not match the selected target")
    index = document.get("index")
    if not isinstance(index, dict):
        raise InputError("rustdoc JSON item index is missing")
    paths = document.get("paths")
    if not isinstance(paths, dict):
        raise InputError("rustdoc JSON path table is missing")
    exports = module_exports(document, args.package_name)
    records: list[dict] = []
    trait_owner: dict[str, str] = {}
    inherent_owner: dict[str, str] = {}
    sealed_traits: set[str] = set()
    for owner_id, item in index.items():
        inner = item.get("inner", {})
        if "trait" in inner:
            if is_public(item) and trait_is_sealed(item, index, exports):
                sealed_traits.add(str(owner_id))
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
                if not is_public(owner):
                    continue
                owner_paths = exports.get(owner_id, set())
                if not owner_paths:
                    continue
                is_trait_method = str(item_id) in trait_owner
                method_kind = "trait_method" if is_trait_method else "inherent_method"
                for owner_path in owner_paths:
                    records.append(function_record(
                        owner_path + (item.get("name", "?"),), item, method_kind, document, exports,
                        args, owner_id=owner_id, inherited_public=is_trait_method,
                        sealed=owner_id in sealed_traits,
                    ))
            elif is_public(item):
                for path in exports.get(str(item_id), set()):
                    records.append(function_record(path, item, "function", document, exports, args))
        elif kind == "associated_type":
            owner_id = trait_owner.get(str(item_id))
            owner = index.get(owner_id or "", {})
            if owner_id and is_public(owner):
                for owner_path in exports.get(owner_id, set()):
                    records.append(associated_type_record(
                        owner_path + (item.get("name", "?"),), item, document, exports, args,
                        owner_id, owner_id in sealed_traits,
                    ))

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
    types = build_type_closures(unique, document, exports, trait_owner, args)
    total_doc_bytes = sum(
        len((record.get("docs") or "").encode()) for record in unique
    ) + sum(len((record.get("docs") or "").encode()) for record in types)
    if total_doc_bytes > MAX_TOTAL_DOC_BYTES:
        raise InputError("selected API documentation exceeds the total index bound; select fewer paths")
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
        "stable_rustc_version": args.stable_rustc_version,
        "extractor": {
            "mode": "nightly-rustdoc-json",
            "executable_sha256": extractor_digest,
            "rustc_version": args.rustdoc_version,
            "rustdoc_format": f"rustdoc-json:{args.rustdoc_format_version}",
        },
        "items": unique,
        "types": types,
        "limits": {
            "max_items": MAX_ITEMS,
            "max_types": MAX_TYPES,
            "max_type_depth": MAX_TYPE_DEPTH,
            "max_type_references": MAX_TYPE_REFERENCES,
            "max_index_bytes": MAX_INDEX_BYTES,
            "max_doc_bytes": MAX_DOC_BYTES,
            "max_total_doc_bytes": MAX_TOTAL_DOC_BYTES,
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
    parser.add_argument("--stable-rustc-version", required=True)
    parser.add_argument("--source-root", required=True, type=pathlib.Path)
    parser.add_argument("--rustdoc-version", required=True)
    parser.add_argument("--rustdoc-format-version", required=True, type=int)
    parser.add_argument("--select", action="append", default=[])
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    try:
        if not args.rustdoc_version.startswith("rustdoc ") or "nightly" not in args.rustdoc_version:
            raise InputError("extractor mode requires the explicitly pinned nightly rustdoc version")
        stable_release = args.stable_rustc_version.split()
        if len(stable_release) < 2 or stable_release[0] != "rustc" or "-" in stable_release[1]:
            raise InputError("selected stable rustc version must be exact and prerelease-free")
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
