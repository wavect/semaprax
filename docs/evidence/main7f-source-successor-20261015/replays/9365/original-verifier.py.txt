"""Finite strict final/branch/legacy source custody; never executes Rust."""
from pathlib import Path
import sys,json,subprocess,hashlib
R=Path.cwd();D=Path(__file__).resolve().parent;sys.path.insert(0,str(R/'docs/evidence/incremental-source-composition-5f1fecd-20261015'));from source_tools import Git
G=Git(R);sha=lambda b:hashlib.sha256(b).hexdigest();read=lambda p:(R/p).read_bytes();load=lambda p:json.loads(read(p));META=json.loads((D/'branches.json').read_text());BASE=META['common_predecessor'];BRANCHES=META['branches'];F='docs/evidence/source-composition-f813-held-20261015/';P='docs/evidence/incremental-source-composition-5f1fecd-20261015/'
def apply(rows,p,current):
 found=[r for r in rows if r['source']==p];assert len(found)<=1
 if not found:return current
 r=found[0];assert sha(current)==r['current_sha256'],(p,'strict current source')
 for e in r['edits']:assert current.count(e['new'].encode())==1;current=current.replace(e['new'].encode(),e['old'].encode(),1)
 assert sha(current)==r['original_sha256'];return current
views={s:json.loads((D/('view-'+s+'.json')).read_text()) for s in BRANCHES}
oldproject={'9d':load('docs/evidence/compiler-receiving-fix-7800-20261015/predecessor-projections.json'),'842':load('docs/evidence/inline-weighted-successor-842/source-inverses.json'),'843':load('docs/evidence/held-closure-receiver-843/runtime-projections.json')}
final_paths=set(META['source_paths']);existing_common=[];strict_negative=0
try:
 for stage,rev in BRANCHES.items():
  inventory=json.loads((D/('view-'+stage+'-inventory.json')).read_text())
  for r in views[stage]:
   p=r['source'];cur=read(p);old=G.get(rev,p);assert old is not None;assert apply([r],p,cur)==old;assert len(r['edits'])==1 and cur.decode()==r['edits'][0]['new']
   if cur!=old:
    try:apply([r],p,old)
    except AssertionError:strict_negative+=1
    else:raise AssertionError('stale named branch raw accepted:'+p)
  for r in inventory['equal']:assert read(r['source'])==G.get(rev,r['source']);assert sha(read(r['source']))==r['sha256']
  for r in inventory['absent']:assert G.get(rev,r['source']) is None and sha(read(r['source']))==r['current_sha256']
 for p in sorted(final_paths):
  if G.get(BASE,p) is None:continue
  expected=G.get(BASE,p);outputs=[]
  for s in BRANCHES:
   branch=apply(views[s],p,read(p));assert branch==G.get(BRANCHES[s],p)
   outputs.append(apply(oldproject[s],p,branch))
  assert outputs[0]==outputs[1]==outputs[2]==expected,p;existing_common.append(p)
 # Complete direct842 and843 source control domains remain original runtime data.
 for row in oldproject['842']:
  p=row['source'];cur=apply(views['842'],p,read(p));assert sha(cur)==row['current_sha256'];assert cur.decode()==row['edits'][0]['new'];assert apply([row],p,cur)==G.get(BASE,p)
 rows843=load('docs/evidence/held-closure-receiver-843/source-inverses.json');assert len(rows843)==11
 for row in rows843:
  p=row['path'];cur=apply(views['843'],p,read(p));assert sha(cur)==row['after_sha256'];assert cur.decode()==row['after']
  if row['before'] is not None:assert apply(oldproject['843'],p,cur).decode()==row['before']
 # Both guarded loop assertion bodies are unmodified; only a declared source-view prefix was inserted.
 for s,p,anchor in [('842','src/cleanup_plan/replay/factored/canonical_shared/inline_weighted/source.rs','    for (path, current) in sources {\n'),('843','src/cleanup_plan/replay/factored/canonical_shared/closure_receiving/source.rs','    for &(path, current) in sources {\n')]:
  prefix='        let projected = crate::source_reflows_merge::assembly_view("'+s+'", path, current);\n        let current = projected.as_str();\n';cur=read(p).decode();assert cur.count(prefix)==1;assert cur.replace(anchor+prefix,anchor,1).encode()==G.get(BRANCHES[s],p)
 # Complete independent receiver reconstruction: exact untouched branch bytes are also final bytes.
 old=(R/'docs/evidence/inline-weighted-successor-842/canonical_shared.rs.before.txt').read_text();start=old.index('type SharedWeighted');end=old.index('\n#[cfg(test)]\nmod tests;',start)
 expected='//! Complete independent 7800 predecessor; original weighted backing and charges.\nuse super::super::{Automaton, BTreeMap, Diagnostic, Label, Rc, ReplayBudget, ResolvedFunction, Weighted, physical_clone, replay_error};\nuse super::{memo, ordered_alias, ordered_weights, successor_targets, app_census, tests};\n\n'+old[start:end]
 for name in ['own_weighted','weighted_successor','share_weighted']:expected=expected.replace('fn '+name+'(', 'pub(super) fn '+name+'(',1)
 expected=expected.replace('    fn successors(', '    pub(super) fn successors(',1)+"\nimpl Canonicalizer<'_> {\n    pub(super) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> { &self.ids }\n    pub(super) fn states(&self) -> &BTreeMap<SharedWeighted, usize> { &self.states }\n}\n"
 assert expected.encode()==read('src/cleanup_plan/replay/factored/canonical_shared/before_inline_weighted.rs')
 old=read('docs/evidence/held-closure-receiver-843/canonical_shared.rs.before.txt').decode();body='type SharedWeighted'+old.split('type SharedWeighted',1)[1].split('\n#[cfg(test)]\nmod tests;',1)[0]
 for a,b in [('    fn closure(','    pub(super) fn closure('),('    fn successors(','    pub(super) fn successors('),('pub(super) struct Canonicalizer','pub(in crate::cleanup_plan::replay::factored) struct Canonicalizer'),('    pub(super) fn new(','    pub(in crate::cleanup_plan::replay::factored) fn new('),('    pub(super) fn id(','    pub(in crate::cleanup_plan::replay::factored) fn id(')]:body=body.replace(a,b)
 oldclone=read('src/cleanup_plan/replay/factored/canonical_shared/before_closure_receiver.rs').decode();assert 'type SharedWeighted'+oldclone.split('type SharedWeighted',1)[1].split("\nimpl Canonicalizer<'_> {",1)[0]==body
 oldcaller=read('docs/evidence/held-closure-receiver-843/factored.rs.before.txt').decode();body=oldcaller.split('fn compare_with_index_storage<',1)[1].split('\nstruct BuiltComparison',1)[0];assert read('src/cleanup_plan/replay/factored/before_closure_invocation.rs').decode().split('pub(super) fn compare_with_index_storage<',1)[1]==body+'\n'
 # Full actual final→7800→38 earlier bindings (all current source owners in every prior packet).
 def base(p):
  b=apply(views['9d'],p,read(p));b=apply(oldproject['9d'],p,b);assert b==G.get(BASE,p),p;return b
 for name in ['current-to-prior.json','main-projections.json','ours-projections.json','pre-ours.json','held-projections.json','weighted-projections.json','source-inverses.json']:
  rows=load(F+name)
  for r in rows:apply([r],r['source'],base(r['source']))
 held=load('docs/evidence/held-auth-839/source-inverses.json');assert len(held)==26
 for r in held:
  b=apply(load(F+'held-projections.json'),r['path'],base(r['path']));assert b.decode()==r['after'];assert sha(b)==r['after_sha256']
  if r['before'] is not None:assert sha(r['before'].encode())==r['before_sha256']
 prior=load(P+'combined-source-proof.json');assert prior['total_packets']==101 and prior['total_rows']==904;cp=load(F+'composition-proof.json');assert cp['total_prior_packets']==101 and cp['total_prior_rows']==904
 joins=load(F+'current-to-prior.json');priorrev=cp['prior_metadata'];owner_count=0
 for entry in prior['packets']:
  p=entry['active_packet'];packet=read(p);assert packet==G.get(priorrev,p);assert sha(packet)==cp['prior_packet_sha256'][p];data=json.loads(packet);rs=data if isinstance(data,list) else data.get('whole_sources',data.get('owners',[data]));assert len(rs)==entry['rows']
  for r in rs:
   p=r.get('source',r.get('file',r.get('path')));assert apply(joins,p,base(p))==G.get(priorrev,p);owner_count+=1
 for p in prior['source_inputs']:assert apply(joins,p,base(p))==G.get(priorrev,p)
 for k in ['preserved_prior_proofs_and_scripts','unchanged_original_archives']:
  for p,h in prior[k].items():assert sha(read(p))==h
 protected={}
 for stage,paths in {'9d':['docs/evidence/compiler-receiving-fix-7800-20261015/predecessor-projections.json'],'842':['docs/evidence/inline-weighted-successor-842/source-inverses.json','docs/evidence/inline-weighted-successor-842/new-source-bindings.json'],'843':['docs/evidence/held-closure-receiver-843/runtime-projections.json','docs/evidence/held-closure-receiver-843/source-inverses.json']}.items():
  for p in paths:assert read(p)==G.get(BRANCHES[stage],p);protected[p]=sha(read(p))
 # Independently authenticate the complete later sticky-correction branch inputs.
 sticky_path='docs/evidence/held-closure-receiver-843-sticky-fix/source-inverses.json';sticky_rev='7d59963a1b841064e6296c138c01810df2b4d587';sticky_rows=load(sticky_path);assert len(sticky_rows)==2;assert read(sticky_path)==G.get(sticky_rev,sticky_path);protected[sticky_path]=sha(read(sticky_path))
 for row in sticky_rows:
  p=row['path'];cur=apply(json.loads((D/'sticky-source-view.json').read_text()),p,read(p));assert cur==G.get(sticky_rev,p);assert cur.decode()==row['after'];assert sha(cur)==row['after_sha256'];assert row['before'].encode()==G.get(BRANCHES['843'],p);assert sha(row['before'].encode())==row['before_sha256']
 # Only these three existing Rust source-custody files may change after the approved merged code.
 changed=subprocess.check_output(['git','diff','--name-only','--diff-filter=M',META['merged_input']],text=True).splitlines();expected=META['original_production_unchanged_except_source_custody_helpers'];assert sorted(changed)==sorted(expected);assert len(expected)==3
 for r in json.loads((D/'immediate-merged-inverse.json').read_text()):assert apply([r],r['source'],read(r['source']))==G.get(META['merged_input'],r['source'])
 proof={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','merged_code_basis':META['merged_input'],'named_branches':BRANCHES,'finite_final_owners':len(final_paths),'common_7800_sources_reconciled':len(existing_common),'strict_stale_raw_negative_checks':strict_negative,'original_direct842_rows':13,'original_direct843_rows':11,'sticky_full_source_rows':2,'held_rows':26,'prior_packets':101,'prior_rows':owner_count,'prior_current_to_prior_joins':len(joins),'all_old_assertion_bodies_and_runtime_domains_preserved':True,'all_production_code_unchanged_from_merged_input':True,'protected_runtime_hashes':protected,'git_cache_peak_bytes':G.peak,'new_runtime_results':0}
 (D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
finally:G.close()
