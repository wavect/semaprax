"""Whole independent checkpoint views only; no old metadata mutation or Rust."""
from pathlib import Path
from functools import lru_cache
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BRANCHES={'796':'dfcf0f36aae041a25bd5fbed87547ea0268a9897','831':'21ac878598f288d6a3d90b984025c034397be2b5','841':'c37b7eea8b680ca4cb63c85758c6d7713745e8d3','845':'36b924b0165b987814c6f2c5dc5f38001ccbcc88'};h=lambda b:hashlib.sha256(b).hexdigest()
# No unbounded Git blob cache; each current/old pair is released per row.
docrows=json.loads((D/'current-docs-to-896.json').read_text());docbefore={r['source']:(R/r['archive']).read_bytes() for r in docrows}
counts={}
for name,rev in BRANCHES.items():
 rows=[];paths=subprocess.check_output(['git','diff','--name-only',rev],cwd=R,text=True).splitlines()
 for p in paths:
  path=R/p
  if not path.is_file():raise AssertionError('deleted source not allowed '+p)
  entry=subprocess.check_output(['git','ls-tree',rev,'--',p],cwd=R,text=True).strip()
  if not entry:continue
  old=subprocess.check_output(['git','show',rev+':'+p],cwd=R);new=docbefore.get(p,path.read_bytes())
  if old==new:continue
  if p.startswith('docs/evidence/'):
   # The845 branch predates integrated MAIN. Preserve the physical current
   # packet unchanged; only a named older-domain alias may differ.
   inherited=subprocess.check_output(['git','show',BRANCHES['796']+':'+p],cwd=R)
   assert new==inherited,'physical inherited evidence altered: '+p
  archive=D/f'{name}-before-{len(rows)}.txt';archive.write_bytes(old)
  rows.append({'source':p,'checkpoint_revision':rev,'archive':str(archive.relative_to(R)),'current_sha256':h(new),'original_sha256':h(old),'edits':[{'new':new.decode(),'old':old.decode()}]})
 (D/f'final-to-{name}.json').write_text(json.dumps(rows,indent=2)+'\n');counts[name]=len(rows)
# Local source inverse is the complete immediate796 parent domain; other named
# rows are independent aliases, not four sequential ownership histories.
(D/'source-inverses.json').write_text(json.dumps(json.loads((D/'final-to-796.json').read_text())+docrows,indent=2)+'\n');(D/'branch-checkpoints.json').write_text(json.dumps(BRANCHES,indent=2)+'\n');print(json.dumps(counts))
