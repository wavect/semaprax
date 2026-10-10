"""Closed actual bundled registry adapter for the two typed public specimens.

No source execution, dependency download, resolver cache, or guessed library list.
Unknown registry shapes and additional-source packages fail closed.
"""
from __future__ import annotations
import os
from pathlib import PurePosixPath
import re

def parse_registry(text: str) -> dict[str, dict[str, object]]:
    if not re.search(r'const VERSION:\s*Version\s*=\s*Version\(0,\s*1,\s*0\);', text):
        raise ValueError('compiled bundled version differs from supported0.1.0')
    normalized = re.sub(r'\s+', '', text)
    if 'letadditional=ifpackage.name=="std.data.csv"{CSV_ADDITIONAL_SOURCES}else{&[]};' not in normalized:
        raise ValueError('unsupported additional bundled source selection')
    section = re.search(r'const PACKAGES: &\[BundledPackage\] = &\[(.*?)\n\];', text, re.S)
    if not section:
        raise ValueError('unsupported bundled registry shape')
    blocks = re.findall(r'BundledPackage\s*\{([^{}]*)\}', section[1], re.S)
    residue = re.sub(r'BundledPackage\s*\{[^{}]*\}\s*,?', '', section[1], flags=re.S)
    if residue.strip() or not blocks:
        raise ValueError('unparsed bundled registry material')
    result = {}
    for block in blocks:
        matched = re.fullmatch(r'\s*name:\s*"([^"\\]+)",\s*path:\s*"([^"\\]+)",\s*'
            r'source:\s*include_str!\("([^"\\]+)"\),\s*dependencies:\s*&\[([^\]]*)\],\s*', block, re.S)
        if not matched:
            raise ValueError('unsupported bundled package entry')
        name, workspace_path, included, dependencies = matched.groups()
        deps = re.findall(r'"([^"\\]+)"', dependencies)
        if re.sub(r'"[^"\\]+"\s*,?', '', dependencies).strip() or len(set(deps)) != len(deps):
            raise ValueError('invalid bundled dependency list')
        source = os.path.normpath(str(PurePosixPath('src/project') / included))
        if not source.startswith('std/') or name in result:
            raise ValueError('duplicate name or escaping bundled source path')
        if not workspace_path.startswith(f'dependencies/{name}/0.1.0/'):
            raise ValueError('bundled workspace path differs from module/version')
        result[name] = {'workspace_path': workspace_path, 'source': source, 'dependencies': sorted(deps)}
    return result


def dependency_names(table: object) -> dict[str, str]:
    """Flatten TOML dotted dependency tables, rejecting duplicate/invalid leaves."""
    if not isinstance(table, dict):
        raise ValueError('dependency table must be a table')
    result: dict[str, str] = {}

    def visit(values: dict, prefix: str) -> None:
        for key, value in values.items():
            if not isinstance(key, str) or not re.fullmatch(r'[a-z][a-z0-9_.]*', key):
                raise ValueError('invalid dependency name component')
            name = prefix + key
            if isinstance(value, dict):
                if not value:
                    raise ValueError('empty nested dependency table')
                visit(value, name + '.')
            elif isinstance(value, str) and name not in result:
                result[name] = value
            else:
                raise ValueError('duplicate or invalid dependency leaf')

    visit(table, '')
    return result


def closure(manifest: dict, registry: dict) -> list[str]:
    if any('dependency' in key and key != 'dependencies' for key in manifest) or manifest.get('resolver'):
        raise ValueError('this adapter only admits closed compiler-bundled dependencies')
    roots = dependency_names(manifest.get('dependencies', {}))
    if not roots or not isinstance(roots, dict) or any(v != '=0.1.0' for v in roots.values()):
        raise ValueError('exact declared bundled version0.1.0 is required')
    pending, selected = list(roots), set()
    while pending:
        name = pending.pop()
        if name in selected:
            continue
        if name not in registry:
            raise ValueError('unclosed or nonbundled dependency')
        if name == 'std.data.csv':
            raise ValueError('additional bundled source package requires its own adapter')
        selected.add(name)
        pending.extend(registry[name]['dependencies'])
    return sorted(selected)


def validate_package(package: dict, manifest: dict, included: str) -> None:
    metadata = manifest.get('package', manifest)
    if metadata.get('version') != '0.1.0':
        raise ValueError('bundled manifest version differs from compiled registry')
    if dependency_names(manifest.get('dependencies', {})) != {name: '=0.1.0' for name in package['dependencies']}:
        raise ValueError('package manifest dependency set differs from compiled registry')
    sources = manifest.get('modules', manifest).get('sources', [])
    if included not in sources:
        raise ValueError('compiled source is not a declared package source')
