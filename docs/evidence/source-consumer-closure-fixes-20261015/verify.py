"""Bounded source-only replay of consumer inverses and production audit scope."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path.cwd();D=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
def load(p):return json.loads((R/p).read_text())
def project(r,s):
 assert sha(s.encode())==r['current_sha256'],r['source']
 for e in r['edits']:
  assert s.count(e['new'])==1,(r['source'],'unique full historical hook')
  s=s.replace(e['new'],e.get('old',e.get('before')),1)
 assert sha(s.encode())==r['original_sha256'],r['source']
 return s
ctx=load(D.relative_to(R)/'contexts.json');checked=0
for domain,p in ctx['consumer_packets'].items():
 active={r['source']:r for r in load(D.relative_to(R)/(domain+'.json'))}
 old=load(p);old=old['owners'] if isinstance(old,dict) else old
 for r in old:
  s=(R/r['source']).read_text()
  if r['source'] in active:s=project(active[r['source']],s)
  project(r,s);checked+=1
for r in load(D.relative_to(R)/'receiving831.json'):project(r,(R/r['source']).read_text())
receiving={r['source']:project(r,(R/r['source']).read_text()) for r in load(D.relative_to(R)/'receiving831.json')}
controls=receiving['src/cleanup_plan/replay/factored/canonical_shared/singleton_target_tests.rs']
assert controls==(R/'docs/evidence/singleton-target-receiving-layers-831/complete-seven-controls-before.rs.txt').read_text()
helpers=receiving['src/cleanup_plan/replay/factored/canonical_shared/successor_targets.rs']
for a,b in [('#[cfg(test)]\nuse super::{','use super::{'),('use std::collections::BTreeMap;\n#[cfg(test)]\nuse std::mem::size_of;','use std::{collections::BTreeMap, mem::size_of};'),('// Complete prior helpers qualify old receiving boundaries only.\n#[cfg(test)]\nimpl Targets {','impl Targets {')]:
 assert helpers.count(a)==1;helpers=helpers.replace(a,b,1)
assert helpers==(R/'docs/evidence/singleton-target-receiving-layers-831/complete-target-helpers-before.rs.txt').read_text()
for r in load(D.relative_to(R)/'current-to-3d4b.json'):
 result=project(r,(R/r['source']).read_text())
 assert result.encode()==subprocess.check_output(['git','show',ctx['basis']+':'+r['source']])
# Replay the real shared CI route using the new raw predecessor, not stale raw acceptance.
def route(rows,p,b):
 rs=[r for r in rows if r['source']==p];assert len(rs)<=1
 return project(rs[0],b) if rs else b
prefix=str(D.relative_to(R))+'/'
consumer=load(prefix+'current-to-3d4b.json');corrective=load('docs/evidence/d1bb-corrective-source-successor-20261015/current-to-d1bb.json')
M='docs/evidence/main7f-source-successor-20261015/';F='docs/evidence/final-842-843-source-assembly-20261015/'
views={v:load(F+'view-'+v+'.json') for v in ['9d','842','843']}
hooks={'9d':load('docs/evidence/compiler-receiving-fix-7800-20261015/predecessor-projections.json'),'842':load('docs/evidence/inline-weighted-successor-842/source-inverses.json'),'843':load('docs/evidence/held-closure-receiver-843/runtime-projections.json')}
ci=load('src/source_reflows.json');assert len(ci)==87
for r in ci:
 p=r['path'];raw=(R/p).read_text();b=route(corrective,p,route(consumer,p,raw));old=route(load(M+'current-to-9365.json'),p,b)
 branches=[route(hooks[v],p,route(views[v],p,old)) for v in views];assert branches[0]==branches[1]==branches[2]
 route(load('docs/evidence/source-composition-f813-held-20261015/main-projections.json'),p,branches[0])
 live=route(load(M+'current-to-main7f.json'),p,b);assert sha(live.encode())==r['after_sha256']
 for e in r['inverse']:assert live.count(e['after'])==1;live=live.replace(e['after'],e['before'],1)
 assert sha(live.encode())==r['before_sha256']
 if p=='src/cleanup_plan/replay/factored/canonical_before.rs':assert '&BTreeMap<(u64, Vec<(Label, usize)>), usize>' in live
ns={};exec((D/'module-parser-original.py.txt').read_text(),ns);ns['R']=R;mask=ns['mask'];regs=[];original=ns['parse']
def logged(p,prefix,chain,kind,package,custom_path=False):
 if p.exists():regs.append({'source':str(p.resolve().relative_to(R)), 'chain':chain})
 return original(p,prefix,chain,kind,package,custom_path)
ns['parse']=logged
logged(R/'src/workspace_graph/expected_projection.rs',['workspace_graph','expected_projection'],[],'lib','semaprax')
# Parse the literal macro bodies, preserving include order and nested macro arms.
def macros(s):
 m=mask(s);out={}
 for z in re.finditer(r'macro_rules!\s+(\w+)\s*\{',m):
  pos=z.end();depth=1;end=pos
  while depth:depth+=(m[end]=='{')-(m[end]=='}');end+=1
  body=s[pos:end-1];bm=mask(body)
  for a in re.finditer(r'\((\w*)\)\s*=>\s*\{',bm):
   start=a.end();j=start;d=1
   while d:d+=(bm[j]=='{')-(bm[j]=='}');j+=1
   out[(z[1],a[1])]=body[start:j-1]
 return out
inv='tests/workspace/semantic_graph/source_inventory.rs';source=(R/inv).read_text();defs=macros(source)
def expand(name,arg='',definitions=defs):
 body=definitions[(name,arg)];events=[]
 for x in re.finditer(r'include_str!\s*\(\s*"([^"]+)"\s*\)|\b(semantic_graph_\w+)!\s*\((\w*)\)',body):
  if x[1]:events.append(x[1][6:] if x[1].startswith('../../') else 'tests/workspace/'+x[1])
  else:events+=expand(x[2],x[3],definitions)
 return events
private=expand('semantic_graph_private_layout_owned_source_inventory')
actual_private={str(p.relative_to(R)) for p in (R/'src/workspace_graph/expected_projection/private_layout').rglob('*.rs')}|{'src/workspace_graph/expected_projection/private_layout.rs'}
assert set(private)==actual_private
purity=expand('semantic_graph_projection_root_source_inventory')
for a in ['local_identity','call_identity','call_sites','cost','declaration_cost','identity_slots','statement_segment','defaults','import_stub','uncached_peak','type_rewrite']:purity+=expand('semantic_graph_projection_child_source_inventory',a)
purity+=private
broad=expand('semantic_graph_private_layout_source_inventory');before=subprocess.check_output(['git','show',ctx['basis']+':'+inv]).decode();oldbroad=expand('semantic_graph_private_layout_source_inventory',definitions=macros(before))
it=iter(broad);assert all(any(v==p for v in it) for p in oldbroad),'every old custody include retains order'
allns={str(p.relative_to(R)) for p in (R/'src/workspace_graph/expected_projection').rglob('*.rs')}|{'src/workspace_graph/expected_projection.rs'}
assert allns<=set(purity+broad), sorted(allns-set(purity+broad))
production={r['source'] for r in regs if not any('#[cfg(test)]' in c['attributes'] for c in r['chain'])}
assert production<=set(purity),(production-set(purity))
excluded=allns-set(purity)
for p in excluded:
 chains=[r['chain'] for r in regs if r['source']==p];assert chains and all(any('#[cfg(test)]' in c['attributes'] for c in ch) for ch in chains),p
joined='\n'.join((R/p).read_text() for p in purity)
assert 'use super::*;' not in joined
prelude=(R/'src/workspace_graph/expected_projection/prelude_forecast_tests.rs').read_text();tls='crate::workspace_graph::ACTIVE_BUILDER_LIMIT';assert joined.count(prelude)==1 and prelude.count(tls)==6
joined=joined.replace(prelude,prelude.replace(tls,'test_only_builder_limit'),1)
for p in ['src/workspace_graph/expected_projection/private_layout/function_runtime.rs','src/workspace_graph/expected_projection/uncached_absence_tests.rs']:joined=joined.replace((R/p).read_text(),'')
for forbidden in ['workspace::','std::fs','std::process','snapshot(','build_authenticated','render_graph_json','write_file','create_dir','remove_file','ACTIVE']:assert forbidden not in joined,forbidden
report={'source_only':True,'historical_consumer_rows_replayed':checked,'full_immediate_sources':len(load(D.relative_to(R)/'current-to-3d4b.json')),'all_namespace_sources':len(allns),'all_namespace_custody_joined':len(allns&set(purity+broad)),'production_parser_paths':sorted(production),'purity_paths':purity,'compiled_test_only_excluded_from_purity':sorted(excluded),'private_layout_joined':len(set(private)),'all_original_broad_includes_retained_in_order':True,'new_rust_test_bodies':0,'live_ci_full_inverse_rows':len(ci),'runtime':'NOT_EXECUTED'}
(D/'source-only-proof.json').write_text(json.dumps(report,indent=2)+'\n');print('PASS source-only:',checked,'historical rows;',len(allns),'namespace files;',len(production),'production paths;',len(excluded),'cfgtest exclusions')
