"""Finite whole-source checks; never executes Rust, Cargo, or source receivers."""
from pathlib import Path
import hashlib,json,subprocess,importlib.util,re
D=Path(__file__).resolve().parent;R=D.parents[2]
O='e031060ce138243b20792779a7f0a0ee4c7d2acb';M='21abcda9bc82864834914dc5b89c91bc5a9c94a5'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(h,p):return subprocess.check_output(['git','show',h+':'+p],cwd=R)
def inverse(row):
 raw=(R/row['path']).read_bytes();assert sha(raw)==row['after_sha256']
 before=raw
 for edit in row['inverse']:
  a=edit['after'].encode();b=edit['before'].encode();assert before.count(a)==1;before=before.replace(a,b,1)
 assert before==(D/row['archive']).read_bytes()==git(row['head'],row['path'])
 assert sha(before)==row['before_sha256'];return before
m=json.loads((D/'input-domains.json').read_bytes());assert len(m['main'])==148 and len(m['ours'])==9
negative=0
for domain,head in [('main',M),('ours',O)]:
 seen=set()
 for row in m[domain]:
  path=row['path'];assert path not in seen;seen.add(path)
  raw=(R/path).read_bytes();view=(R/row['archive']).read_bytes()
  assert len(raw)==row['current_len'] and sha(raw)==row['current_sha256']
  assert len(view)==row['view_len'] and sha(view)==row['view_sha256'] and view==git(head,path)
  assert len(view)<=9382389
  for bad in [raw+b'\n',raw[:-1],b'foreign']:
   assert bad not in (raw,view);negative+=1
  assert sha(view)!='0'*64;negative+=1
# Exercise the exact immutable incoming row algorithms on their full MAIN input,
# keeping edited-source and recomputed-digest archive forgeries distinct.
def restore(proof,raw):
 assert sha(raw)==proof['after_sha256']
 text=raw.decode()
 for edit in proof['inverse']:
  assert text.count(edit['after'])==1
  text=text.replace(edit['after'],edit['before'],1)
 assert sha(text.encode())==proof['before_sha256']
 return text.encode()
original_rows=0
for name in ['views.json','sources.json']:
 proofs=json.loads(git(M,'docs/evidence/borrowed-exit-region-match-v1/'+name))
 for proof in proofs:
  raw=git(M,proof['path']);before=git(M,proof['archive'])
  assert restore(proof,raw)==before
  for wrong in [raw+b'\n',raw[:-1]]:
   try:restore(proof,wrong)
   except AssertionError:negative+=1
   else:raise AssertionError('source mutant accepted')
  forged=dict(proof,before_sha256='0'*64)
  try:restore(forged,raw)
  except AssertionError:negative+=1
  else:raise AssertionError('forged archive digest accepted')
  original_rows+=1
for p in (D/'metadata-ours').rglob('*.json'):
 rel=str(p.relative_to(D/'metadata-ours'));assert p.read_bytes()==(R/rel).read_bytes()==git(O,rel)
 assert (D/'metadata-main'/rel).read_bytes()==git(M,rel)
rows=json.loads((D/'full-source-inverses.json').read_text())
for row in rows:inverse(row)
# Whole incoming program retained at prefix, with only finite acquiring edits.
prefix=(R/'src/source_reflows/incoming21_semantic_receivers.rs').read_text().split('const MAIN21_MANIFEST:')[0]
prefix=prefix.replace('main21_read(', 'std::fs::read_to_string(')
prefix=prefix.replace('../../docs/evidence/main21-exit-region-integration-20261017/main-inputs/docs/evidence/', '../../docs/evidence/')
needle='    let acquired = main21_project(proof["path"].as_str().unwrap(), current);\n    let current = acquired.as_str();\n'
assert prefix.count(needle)==1;prefix=prefix.replace(needle,'',1)
assert prefix.encode()==git(M,'src/source_reflows/semantic_receivers.rs')+b'\n'
# Existing independent MAIN34d reader/cap is byte-exact except one scoped ingress.
p='src/source_reflows/incoming67107_semantic_receivers.rs';value=(R/p).read_text()
needle='    let acquired = super::semantic_receivers::incoming_main21::ours_expected_input(proof, current);\n    let current = acquired.as_str();\n'
assert value.count(needle)==1;assert value.replace(needle,'',1).encode()==git(O,p)
assert 'const MAIN34D_CAP: usize = 9_317_546;' in value
# Selected/Oracle own original suffix, including all old checks and payments.
for path in ['src/cleanup_plan/replay/expression_facts/receiving/remaining.rs','src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs']:
 old=git(O,path).decode();now=(R/path).read_text()
 start=old.index('    if !budget.pays_allocations() {');depth=0
 for at in range(start,len(old)):
  if old[at]=='{':depth+=1
  if old[at]=='}':
   depth-=1
   if depth==0:break
 suffix=old[at+1:];assert now.endswith(suffix)
 assert now.index('helpers::EXIT_SELECTOR_BYTES')<now.index('if !matches!(exit.continuation, ExitContinuation::LoopLatch')
# Unchanged strict token parser restores each COMPLETE a82 carrier, sentinels included.
spec=importlib.util.spec_from_file_location('guard',R/'docs/evidence/main043-independent-custody-20261016/guard-extensions.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
allowed=json.loads((R/'docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json').read_text())
from collections import Counter
raw=[]
for path in allowed:
 current=(R/path).read_text();old=git('a82e00cc8',path).decode();fragments=g.fragments(path,current,old);counts=Counter()
 for fragment in fragments:
  for token in re.findall(r'include_str!\("([^"\n]+)"\)',fragment):
   base=(R/path).parent if path=='tests/hir_module_boundaries.rs' else (R/path).parent.parent
   counts[str((base/token).resolve().relative_to(R))]+=1
 rule=allowed[path];assert set(counts)<=set(rule['maximum'])
 assert all(rule['required'].get(k,0)<=counts[k]<=v for k,v in rule['maximum'].items())
 raw.append({'path':path,'fragments':len(fragments),'original_sha256':sha(old.encode()),'current_sha256':sha(current.encode())})
for path in ['src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs','src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs','src/source_reflows/incoming21_semantic_receivers.rs']:
 assert len((R/path).read_text().splitlines())<=1500
result={'status':'PASS_PURE_SOURCE_NO_RUST_EXECUTION','main_domains':148,'ours_domains':9,'whole_source_inverses':len(rows),'original_main_inverse_rows':original_rows,'source_mutation_controls':negative,'whole_raw_guards':raw,'no_typecheck_or_runtime':True}
print(json.dumps(result,indent=2))
