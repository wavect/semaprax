"""Exact source custody only. Does not compile or execute compiler code."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parents[3]
packet = Path(__file__).resolve().parent
sha = lambda text: hashlib.sha256(text.encode()).hexdigest()
records = json.loads((packet / 'exact-source-inverses.json').read_text())
for record in records:
    text = (root / record['source']).read_text()
    assert sha(text) == record['current_sha256'], record['source']
    for edit in record['edits']:
        assert text.count(edit['new']) == 1, record['source']
        text = text.replace(edit['new'], edit['before'], 1)
    assert text == (root / record['original']).read_text()
    assert sha(text) == record['before_sha256']

prior = json.loads((packet / 'legacy-inverses.before.json').read_text())
active_path = root / 'docs/evidence/legacy-linked-drop-reuse-828f/exact-source-inverses.json'
active = json.loads(active_path.read_text())
restored = json.loads(active_path.read_text())
subject = 'src/hir/declaration_index/legacy_drop/tests.rs'
record = next(record for record in restored if record['source'] == subject)
original_record = next(record for record in prior if record['source'] == subject)
assert record['edits'][1:] == original_record['edits']
record['edits'].pop(0)
record['current_sha256'] = original_record['current_sha256']
assert restored == prior
for record in active:
    text = (root / record['source']).read_text()
    assert sha(text) == record['current_sha256'], record['source']
    for edit in record['edits']:
        assert text.count(edit['new']) == 1, record['source']
        text = text.replace(edit['new'], edit['before'], 1)
    assert sha(text) == record['original_sha256'], record['source']
    assert text == (root / record['original']).read_text()

leaf = root / 'src/hir/declaration_index/leaf_drop_tests.rs'
before = (packet / 'original/src/hir/declaration_index/leaf_drop_tests.rs').read_text()
def old_body(text):
    return text.split('fn old_drop(', 1)[1].split('\n#[test]', 1)[0]
assert old_body(leaf.read_text()) == old_body(before)
static = json.loads((root / 'docs/evidence/default-static-error-custody-825/exact-source-inverse.json').read_text())
assert len(static['edits']) == 13
assert static['edits'][12]['name'] == 'ordered-large-integrated-whole-source-preinverse'
assert sha(static['edits'][12]['after']) == '2485211074d2424a39b29dcf1ffaee1c48807c7a0368a211395c367162b8cedb'
assert sha(static['edits'][12]['before']) == 'e97250b91fc9c8bb8f60e6ea1ef7d90a332dcb9dcf3fffdbd66348a9f3e2a366'
text = (root / static['source']).read_text()
for edit in reversed(static['edits']):
    assert text.count(edit['after']) == 1
    text = text.replace(edit['after'], edit['before'], 1)
assert sha(text) == static['before_sha256']
assert text == (root / 'docs/evidence/default-static-error-custody-825/defaults.before.txt').read_text()
print('PASS: 4 immediate whole-source inverses; all 10 inherited legacy inverses; unchanged complete old leaf helper; exact 13-adapter static source restoration')
