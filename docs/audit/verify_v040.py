#!/usr/bin/env python3
"""Read-only documentation consistency and preservation checks.

Run from the repository: python3 docs/audit/verify_v040.py
Use --baseline <commit> to verify unchanged examples, embedded guidance,
historical records and pre-existing non-Markdown evidence against that commit.
This does not execute the compiler or infer target/runtime conformance.
"""
from __future__ import annotations
import argparse
import collections
import hashlib
import html
import json
from pathlib import Path
import re
import subprocess
import sys

FENCE = re.compile(r'^```[^\n]*\n[\s\S]*?^```[ \t]*$', re.MULTILINE)
EMBEDDED = {
    'docs/AGENT-QUICK-REFERENCE.md',
    'docs/LANGUAGE-SHAPES-CATALOG.md',
    'docs/LANGUAGE-SHAPES-CATALOG.json',
    'docs/STANDARD-LIBRARY-CATALOG.md',
    'docs/AGENT-DIAGNOSTIC-HELP.json',
}


def git(root: Path, *args: str) -> bytes:
    return subprocess.check_output(['git', *args], cwd=root)


def targets(source: str) -> list[str]:
    # Same simple local-link selection as tests/documentation.rs.
    return [value.strip('<>') for value in re.findall(r'\]\(([^)]+)\)', source)]


def heading_ids(source: str) -> set[str]:
    source = FENCE.sub('', source)
    values = set(re.findall(r'(?:id|name)=["\']([^"\']+)', source))
    counts: collections.Counter[str] = collections.Counter()
    for match in re.finditer(r'^#{1,6}\s+(.+?)(?:\s+#+)?\s*$', source, re.MULTILINE):
        title = re.sub(r'\[([^\]]+)\]\([^)]*\)', r'\1', match[1])
        title = html.unescape(re.sub(r'<[^>]+>', '', title))
        slug = re.sub(r'[^\w\-\s]', '', title.lower())
        slug = re.sub(r'\s', '-', slug)
        occurrence = counts[slug]
        counts[slug] += 1
        values.add(slug + (f'-{occurrence}' if occurrence else ''))
    return values


def historical(path: str) -> bool:
    name = Path(path).name
    return ('/evidence/' in path or '/decisions/' in path
            or name in {'CHANGELOG-ARCHIVE.md', 'DOCTOR-PROVISIONED-LINUX-GATE-V1.md'}
            or name.startswith('GRAPH-OPERATIONAL-') and 'EXECUTION-EVIDENCE' in name)


def verify(root: Path, baseline: str | None) -> dict:
    docs = root / 'docs'
    pages = sorted(docs.rglob('*.md'))
    errors: list[str] = []
    summary = (docs / 'SUMMARY.md').read_text(encoding='utf-8')
    for target in targets(summary):
        if target.startswith(('https://', 'http://')):
            errors.append(f'docs/SUMMARY.md: mdBook chapter must use a local path: {target}')
    anchor_map = {p.resolve(): heading_ids(p.read_text(encoding='utf-8')) for p in pages}
    links = anchors = blocks = historical_count = immutable_count = 0
    for page in pages:
        text = page.read_text(encoding='utf-8')
        name = page.relative_to(root).as_posix()
        first = text.splitlines()[:12]
        if not first or not first[0].startswith('# '):
            errors.append(f'{name}: missing H1')
        for label in ['Status:', 'Audience:']:
            if not any(line.removeprefix('- ').startswith(label) for line in first):
                errors.append(f'{name}: {label} outside first 12 lines')
        if page.name != 'SUMMARY.md':
            link = '](' + page.relative_to(docs).as_posix() + ')'
            if summary.count(link) != 1:
                errors.append(f'{name}: catalog entries {summary.count(link)}, expected 1')
        for target in targets(text):
            if target.startswith(('http:', 'https:', 'mailto:')):
                continue
            f, _, fragment = target.partition('#')
            selected = (page.parent / f).resolve() if f else page.resolve()
            if f:
                links += 1
                if not selected.exists():
                    errors.append(f'{name}: missing local target {target}')
            if fragment and selected in anchor_map:
                anchors += 1
                if fragment not in anchor_map[selected]:
                    errors.append(f'{name}: unknown chapter anchor {target}')
        blocks += len(FENCE.findall(text))
    # Root links are also covered by the repository's original documentation test.
    for name in ['AGENTS.md','CLAUDE.md','CHANGELOG.md','CONTRIBUTING.md','README.md','SECURITY.md']:
        page = root / name
        for target in targets(page.read_text(encoding='utf-8')):
            if target.startswith(('https:', 'http:', '#', 'mailto:')):
                continue
            f = target.split('#')[0]
            if f and not (page.parent / f).exists():
                errors.append(f'{name}: missing local target {target}')
    # Keep executable tour excerpts tied to their actual committed sources.
    tour = (docs / 'LANGUAGE-TOUR.md').read_text(encoding='utf-8').replace('\r\n', '\n')
    lines = tour.splitlines()
    tour_blocks = 0
    for i, line in enumerate(lines):
        if line.strip() != '```semaprax':
            continue
        j = i
        while j and not lines[j-1].strip():
            j -= 1
        citation = targets(lines[j-1]) if j else []
        citation = [t for t in citation if t.startswith('../examples/') and t.endswith('.spx')]
        end = next((k for k in range(i+1, len(lines)) if lines[k].rstrip() == '```'), None)
        if len(citation) != 1 or end is None:
            errors.append(f'language tour line {i+1}: missing exact citation/fence')
            continue
        excerpt = '\n'.join(lines[i+1:end])
        source = (docs / citation[0]).read_text(encoding='utf-8').replace('\r\n', '\n')
        if not excerpt.strip() or excerpt not in source:
            errors.append(f'language tour line {i+1}: excerpt/source mismatch')
        tour_blocks += 1
    if tour_blocks < 12:
        errors.append(f'language tour: only {tour_blocks} verified excerpts')
    if baseline:
        paths = git(root, 'ls-tree', '-r', '--name-only', baseline, '--', 'docs/').decode().splitlines()
        for name in paths:
            page = root / name
            if not page.is_file():
                errors.append(f'baseline file removed: {name}')
                continue
            old = git(root, 'show', f'{baseline}:{name}')
            new = page.read_bytes()
            preserve = name in EMBEDDED or historical(name) or not name.endswith('.md')
            if preserve:
                immutable_count += 1
                historical_count += int(historical(name))
                if old != new:
                    errors.append(f'preserved file changed: {name}')
            if name.endswith('.md') and FENCE.findall(old.decode('utf-8')) != FENCE.findall(new.decode('utf-8')):
                errors.append(f'worked code/example fences changed: {name}')
    result = {
        'schema': 'semaprax.documentation-audit-check.v1',
        'markdown_files': len(pages), 'local_links_checked': links,
        'chapter_anchors_checked': anchors, 'fenced_blocks': blocks,
        'language_tour_excerpts': tour_blocks,
        'baseline': baseline, 'preserved_files_checked': immutable_count,
        'historical_files_checked': historical_count,
        'compiler_execution': False, 'target_conformance_execution': False,
        'errors': errors, 'passed': not errors,
    }
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--baseline')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = verify(args.root.resolve(), args.baseline)
    rendered = json.dumps(result, indent=2, ensure_ascii=False) + '\n'
    if args.output:
        args.output.write_text(rendered, encoding='utf-8')
    print(rendered, end='')
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
