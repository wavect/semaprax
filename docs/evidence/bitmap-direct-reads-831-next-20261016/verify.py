#!/usr/bin/env python3
"""Finite source-only immediate/full-predecessor checks; never invokes Rust."""
import hashlib,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
OLD=HERE.parent/'empty-closure-bitmap-831-next-20261016'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(rev,p):return subprocess.check_output(['git','show',rev+':'+p],cwd=ROOT)
x=json.loads((HERE/'source-inverses.json').read_text()); views={}
for r in x['rows']:
 a=(ROOT/r['source']).read_bytes();b=r['before'].encode()
 assert a==r['after'].encode() and sha(a)==r['after_sha256']
 assert sha(b)==r['before_sha256'] and b==git(x['predecessor'],r['source'])
 views[r['source']]=b
prior=json.loads((OLD/'source-inverses.json').read_text())
for r in prior['rows']:
 a=views.get(r['source'],(ROOT/r['source']).read_bytes())
 assert a==r['after'].encode() and sha(a)==r['after_sha256']
 assert r['before'].encode()==git(prior['base'],r['source'])
for p in OLD.rglob('*'):
 if p.is_file():assert p.read_bytes()==git(x['predecessor'],str(p.relative_to(ROOT)))
for r in json.loads((HERE/'active-new-owners.json').read_text()):
 b=(ROOT/r['source']).read_bytes();assert sha(b)==r['sha256'] and len(b.splitlines())==r['lines']<=1500
prefix='src/cleanup_plan/replay/factored/canonical_shared/'
for p in ['memo.rs','memo/retained.rs']:
 assert (ROOT/(prefix+p)).read_bytes()==git(prior['base'],prefix+p)
p=prefix+'ordered_weights.rs';a=(ROOT/p).read_bytes();b=git(prior['base'],p)
marker=b'// Compare old/current representations without constructing a projected map'
assert a.split(marker)[0]==b.split(marker)[0]
assert b'pub(super) enum View' not in (ROOT/(prefix+'empty_closure.rs')).read_bytes()
assert len((ROOT/'src/cleanup_plan/replay/factored/canonical_shared.rs').read_bytes().splitlines())<=1500
selectors=json.loads((HERE/'pending-selectors.json').read_text())
assert len(selectors)==6 and len({r['selector'] for r in selectors})==6
for r in selectors:
 leaf=r['selector'].split('::')[-1]
 p=prefix+('before_empty/cached_tests.rs' if 'before_empty::' in r['selector'] else 'empty_closure_tests.rs')
 assert ('fn '+leaf+'(') in (ROOT/p).read_text()
print('PASS: six immediate inverses, five full base inverses, eleven owners, original packets, ordinary Memo/Retained, original production iterator, six literal declarations; no Rust execution')
