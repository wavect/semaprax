"""Complete source inverses and unchanged assertions; never executes Rust/Node."""
from pathlib import Path
import hashlib,json
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
rows=json.loads((D/'source-inverses.json').read_text())
for r in rows:
 current=(R/r['source']).read_bytes();old=(R/r['archive']).read_bytes()
 assert sha(current)==r['current_sha256'] and sha(old)==r['original_sha256']
 text=current.decode()
 for e in r['edits']:
  assert text.count(e['new'])==1;text=text.replace(e['new'],e['before'],1)
 assert text.encode()==old
old=(D/'original/0.before').read_text();new=(R/rows[0]['source']).read_text()
a=old.index('    let freezes = sum\n',old.index('fn nested_byte_ancestor_freezes'));b=old.index('    for id in outer.shape.frozen {',a)
# Exact old assertion block, with only one structural Rust indentation level.
assert ''.join('    '+line if line.strip()else line for line in old[a:b].splitlines(True)) in new
suffix=old[b:old.index('#[test]\nfn nested_byte_census_uses',b)]
assert suffix in new, 'complete borrowed ancestor forged-subtree body remains byte exact'
for name in ['const SOURCE:','const ROOTED:']:
 before=old[old.index(name):old.index(';\n',old.index(name))+2]
 assert before in new, 'immutable full original fixture literal preserved'
host=(R/rows[2]['source']).read_text();previous=(D/'original/2.before').read_text()
for statement in ["assert.equal(status,9,'requires status contract.v1/code1');","assert.equal(++reports,1,'exactly one selected report');","assert.equal(owners.size,0,'report follows complete non-result cleanup');","throw Error('contract');","assert.equal(owners.size,0,'all reached owned carriers settle');","assert.equal(allocated,expectedAllocated);"]:
 assert statement in host and statement in previous
assert "const expectedCopies=kind==='success'?2:kind==='failure'||catalog?1:0;"in host
assert "if(nestedFailure){assert.equal(copies,0);assert.equal(drops,0);assert.equal(allocated,0)}"in host
assert "assert.equal(copies,expectedCopies);assert.equal(drops,expectedCopies);"in host
print('PASS4 complete immutable source inverses; every original2-loan/endpoint assertion retained; borrowed forged-subtree suffix/fixtures exact; original catalog ownership and full host report/drop checks retained. SOURCE ONLY, no runtime.')
