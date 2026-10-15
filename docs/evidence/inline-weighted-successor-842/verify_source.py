#!/usr/bin/env python3
"""Bounded source/inverse checks only; never invokes a Rust executable."""
from pathlib import Path
import hashlib,io,json,re,runpy,subprocess,sys
R=Path.cwd();D=Path(__file__).resolve().parent;BASE='7800b014151b6ce0709c351ccdfd52ae25832e07'
sha=lambda b:hashlib.sha256(b).hexdigest()
sys.path.insert(0,str(R/'docs/evidence/incremental-source-composition-5f1fecd-20261015'))
from source_tools import Git
G=Git(R)
changed=subprocess.check_output(['git','diff','--name-only','--diff-filter=M',BASE],text=True).splitlines()
all_changes=subprocess.check_output(['git','diff','--name-only',BASE],text=True).splitlines()
assert all(not p.startswith('docs/evidence/') or p.startswith('docs/evidence/inline-weighted-successor-842/') for p in all_changes),'all old evidence byte-exact'
assert not subprocess.check_output(['git','diff','--name-only','--diff-filter=D',BASE],text=True).strip(),'no source/old-proof deletions'
assert all(not p.startswith('docs/evidence/') for p in changed),'old evidence stays immutable'
assert all('/memo/' not in p and not p.endswith('/memo.rs') and '/physical_budget' not in p for p in changed)
rows=[]
try:
 for source in changed:
  old=G.get(BASE,source);current=(R/source).read_bytes();assert old is not None
  row=dict(source=source,checkpoint_revision=BASE,current_sha256=sha(current),original_sha256=sha(old),edits=[dict(new=current.decode(),old=old.decode())])
  assert current.count(row['edits'][0]['new'].encode())==1
  assert current.replace(current,old,1)==G.get(BASE,source)
  rows.append(row)
 (D/'source-inverses.json').write_text(json.dumps(rows,indent=2)+'\n')
 sources=['src/cleanup_plan/replay/factored/canonical_shared/'+s for s in ['before_inline_weighted.rs','inline_weighted.rs','inline_weighted/tests.rs','inline_weighted/source.rs']]
 new=[]
 for source in sources:
  assert G.get(BASE,source) is None
  current=(R/source).read_bytes();assert len(current.splitlines())<=1500
  new.append(dict(source=source,new_file=True,current_sha256=sha(current),bytes=len(current)))
 (D/'new-source-bindings.json').write_text(json.dumps(new,indent=2)+'\n')
 old=(D/'canonical_shared.rs.before.txt').read_bytes();assert old==G.get(BASE,'src/cleanup_plan/replay/factored/canonical_shared.rs')
 s=old.decode();start=s.index('type SharedWeighted');end=s.index('\n#[cfg(test)]\nmod tests;',start)
 expected='//! Complete independent 7800 predecessor; original weighted backing and charges.\nuse super::super::{Automaton, BTreeMap, Diagnostic, Label, Rc, ReplayBudget, ResolvedFunction, Weighted, physical_clone, replay_error};\nuse super::{memo, ordered_alias, ordered_weights, successor_targets, app_census, tests};\n\n'+s[start:end]
 for name in ['own_weighted','weighted_successor','share_weighted']:expected=expected.replace('fn '+name+'(', 'pub(super) fn '+name+'(',1)
 expected=expected.replace('    fn successors(', '    pub(super) fn successors(',1)
 expected+='\nimpl Canonicalizer<\'_> {\n    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> { &self.ids }\n    pub(super) fn states(&self) -> &BTreeMap<SharedWeighted, usize> { &self.states }\n}\n'
 assert expected==(R/sources[0]).read_text(),'complete immediate independent receiver'
 # Complete existing original receivers and authorities retain every physical byte.
 independent=['src/cleanup_plan/replay/factored/canonical_before.rs']+['src/cleanup_plan/replay/factored/canonical_shared/'+p for p in ['before_alias.rs','before_index.rs','before_ordered.rs','before_scratch.rs','before_scratch_memo.rs','before_targets.rs','before_receiving_layers.rs','before_held.rs','before_held_memo.rs','before_held_retained.rs','memo.rs','memo/retained.rs','ordered_weights.rs','ordered_alias.rs']]
 for source in independent:assert (R/source).read_bytes()==G.get(BASE,source)
 recorded={}
 for line in (R/'tests/module-size-budget.tsv').read_text().splitlines():
  if line and not line.startswith('#'):p,n=line.split('\t');recorded[p]=int(n)
 for source in changed+sources:
  if source.endswith('.rs'):assert len((R/source).read_bytes().splitlines())<=recorded.get(source,1500),(source,'module size')
 protected=['docs/evidence/held-auth-839/source-inverses.json','src/source_reflows.json','src/cleanup_plan/replay/live_oracle/source_custody/presence.json']
 for source in protected:assert (R/source).read_bytes()==G.get(BASE,source)
 for audit,prefix in [('tests/hir_module_boundaries.rs','../'),('tests/workspace/semantic_graph/source_inventory.rs','../../'),('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','../../../../../')]:
  text=(R/audit).read_text()
  for source in sources:assert text.count('include_str!("'+prefix+source+'")')==1
 # Resolve every newly authored include literally, without executing compiler/tools.
 for source in sources:
  p=R/source;text=p.read_text()
  for target in re.findall(r'include_str!\("([^\"]+)"\)',text):assert (p.parent/target).is_file(),(source,target)
 # Both old and new state-key Ord use the exact ordered slice, without shape tags.
 small=(R/sources[1]).read_text();assert 'self.as_slice().cmp(other.as_slice())' in small and 'self.as_slice() == other.as_slice()' in small
 canonical=(R/'src/cleanup_plan/replay/factored/canonical_shared.rs').read_text();one=canonical.split('successor_targets::Targets::One { child, weight } => {',1)[1].split('successor_targets::Targets::Many(targets) => {',1)[0]
 assert 'Vec::' not in one and 'reserve_array' not in one and 'SmallWeighted::One([(child, weight)])' in one
 assert one.index('"factored inline weighted enum and array receiving"')<one.index('SmallWeighted::One(')
 assert one.index('rc_backing_bytes::<SmallWeighted>')<one.index('Rc::new(singleton)')
 # Outboard mirrors run unchanged original guard bodies in their named 7800 stage.
 rb=Path.read_bytes;rt=Path.read_text;op=Path.open
 transport={r['source']:r for r in rows}
 def unpatchedread(p):
  with op(p,'rb') as f:return f.read()
 def projected(p):
  try:key=str(p.resolve().relative_to(R))
  except ValueError:return None
  if key not in transport:return None
  r=transport[key];current=unpatchedread(p);assert sha(current)==r['current_sha256'];assert current.decode()==r['edits'][0]['new'];old=r['edits'][0]['old'].encode();assert sha(old)==r['original_sha256'];return old
 def readbytes(p):
  b=projected(p);return unpatchedread(p) if b is None else b
 def readtext(p,*a,**kw):return readbytes(p).decode(kw.get('encoding') or 'utf-8')
 def openfile(p,mode='r',*a,**kw):
  if mode in ('r','rb'):
   b=projected(p)
   if b is not None:return io.BytesIO(b) if mode=='rb' else io.StringIO(b.decode())
  return op(p,mode,*a,**kw)
 Path.read_bytes=readbytes;Path.read_text=readtext;Path.open=openfile
 import contextlib
 output=io.StringIO()
 try:
  with contextlib.redirect_stdout(output):
   ns=runpy.run_path(str(R/'docs/evidence/source-composition-f813-held-20261015/verify_source36.py'));assert len(ns['results'])==36
   runpy.run_path(str(R/'docs/evidence/source-composition-f813-held-20261015/verify_838.py'))
 finally:Path.read_bytes=rb;Path.read_text=rt;Path.open=op
 (D/'prior-guard-execution.log').write_text(output.getvalue())
 proof=dict(status='SOURCE_ONLY_NO_RUST_EXECUTION',basis=BASE,changed_existing_sources=len(rows),new_owners=len(new),complete_immediate_receiver=True,unchanged_independent_owners=len(independent),source36_originals=36,main_reflows=85,presence_rows=2,old838_forward_inverse_pairs=14,old838_immutable_patch_archive_files=28,protected_inputs={p:sha((R/p).read_bytes()) for p in protected},git_cache_peak_bytes=G.peak,runtime_qualification_pending=True)
 (D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
finally:G.close()
