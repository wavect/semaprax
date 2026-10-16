"""Source-only exact historical read-domain derivation; no Rust execution."""
from pathlib import Path
import argparse,hashlib,json,re,subprocess
p=argparse.ArgumentParser();p.add_argument('--write-endpoints',action='store_true');a=p.parse_args();R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='787f84774e935833cd0d8de17c6c8da21d7b4986';sha=lambda b:hashlib.sha256(b).hexdigest()
def git(ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=R)
def emit(path,value):
 raw=(json.dumps(value,indent=2)+'\n').encode()
 if a.write_endpoints:path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(raw)
 else:assert path.read_bytes()==raw,('stale endpoint',path)
def inverse(row,current):
 assert sha(current.encode())==row['current_sha256']
 for e in row['edits']:assert current.count(e['new'])==1;current=current.replace(e['new'],e['old'],1)
 assert sha(current.encode())==row['original_sha256'];return current
proof=[];specs=[];allpaths=set();audit=[]
for name,table,archivebase,count in [('Compact7','compact-inverse7.json','compact-staged-observations-v1',7),('Receivers12','receiver-inverse12.json','receiver-source-custody-v1',12)]:
 tablepath='docs/evidence/main8a-independent-custody-20261016/'+table;raw=(R/tablepath).read_bytes();assert raw==git(BASE,tablepath);original=json.loads(raw);assert len(original)==count;rows=[]
 for row in original:
  path=row['path'];allpaths.add(path);archive='docs/evidence/'+archivebase+'/'+row['archive'];before=(R/archive).read_bytes();assert before==git(BASE,archive);assert sha(before)==row['before_sha256'];text=before.decode();coordinates={}
  if any(not e['before']for e in row['inverse']):
   witness='docs/evidence/main8a-independent-custody-20261016/before/Main8a/'+path;w=(R/witness).read_bytes();assert w==git(BASE,witness);assert sha(w)==row['after_sha256'];forward=w.decode()
   for i,e in enumerate(row['inverse']):assert forward.count(e['after'])==1;coordinates[i]=forward.index(e['after']);forward=forward.replace(e['after'],e['before'],1)
   assert forward.encode()==before
  for i in reversed(range(len(row['inverse']))):
   e=row['inverse'][i]
   if e['before']:assert text.count(e['before'])==1;text=text.replace(e['before'],e['after'],1)
   else:
    at=coordinates[i];text=text[:at]+e['after']+text[at:]
  assert sha(text.encode())==row['after_sha256'];expected=text
  # Replay every complete immutable inverse and archived predecessor, in its original order.
  for e in row['inverse']:assert text.count(e['after'])==1;text=text.replace(e['after'],e['before'],1)
  assert text.encode()==before
  target=D/'historical-after'/path
  if target.exists():assert target.read_text()==expected
  elif a.write_endpoints:target.parent.mkdir(parents=True,exist_ok=True);target.write_text(expected)
  else:raise AssertionError(('missing immutable target',path))
  current=(R/path).read_text();binding=dict(source=path,current_sha256=sha(current.encode()),original_sha256=sha(expected.encode()),edits=[dict(new=current,old=expected)]if current!=expected else[]);assert inverse(binding,current)==expected
  for wrong in [current+' ','',current[:len(current)//2]]:assert sha(wrong.encode())!=binding['current_sha256']
  if expected!=current:assert sha(expected.encode())!=binding['current_sha256']
  rows.append(binding);proof.append(dict(domain=name,path=path,current_sha256=binding['current_sha256'],historical_after_sha256=row['after_sha256'],archive_before_sha256=row['before_sha256'],reverse_operations=len(row['inverse']),empty_before_coordinates=coordinates))
  # Source-only audit of the actual pre-fix serial dispatch, using baseline complete raw inputs.
  pre=git(BASE,path).decode();stage='raw';problem=None
  for tab in ['docs/evidence/a3-e47-final-custody-20261016/current-to-Previous16c.json','docs/evidence/main8a-independent-custody-20261016/current-toMain8a.json']:
   prior=next((r for r in json.loads(git(BASE,tab))if r['source']==path),None)
   if prior:
    if sha(pre.encode())!=prior['current_sha256']:problem=dict(stage=tab,actual_sha256=sha(pre.encode()),required_sha256=prior['current_sha256']);break
    pre=inverse(prior,pre);stage=tab
  audit.append(dict(domain=name,path=path,failed_stage=problem,serial_result_sha256=sha(pre.encode()),required_owner_after_sha256=row['after_sha256'],old_serial_chain_matches=problem is None and sha(pre.encode())==row['after_sha256']))
 namepath=D/(name+'.json');emit(namepath,rows);payload=namepath.read_bytes()
 specs.append('Self::'+name+' => Spec { data: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/'+str(namepath.relative_to(R))+'")), length: '+str(len(payload))+', sha256: '+str(list(hashlib.sha256(payload).digest()))+', rows: '+str(count)+' },')
helper=R/'src/source_reflows_merge/main8a_historical.rs';source=helper.read_text();updated=re.sub(r'// SPECS_BEGIN.*?// SPECS_END','// SPECS_BEGIN\n'+ '\n'.join(specs)+'\n            // SPECS_END',source,flags=re.S);inputs='\n'.join('"'+path+'" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/'+path+'")),'for path in sorted(allpaths));updated=re.sub(r'// INPUTS_BEGIN.*?// INPUTS_END','// INPUTS_BEGIN\n'+inputs+'\n            // INPUTS_END',updated,flags=re.S)
if a.write_endpoints:helper.write_text(updated)
else:assert source==updated,'stale sealed metadata descriptors'
# Original owners differ only at their named input front. All predicates/hostiles remain exact.
for path,old,new in [('src/source_reflows/semantic_receivers_main8a.rs','crate::source_reflows_merge::main8a_input(path, current)','crate::source_reflows_merge::main8a_historical::input(\n        crate::source_reflows_merge::main8a_historical::Domain::Receivers12, path, current,\n    )'),('src/cleanup_plan/replay/compact_stage/source_custody_main8a.rs','crate::source_reflows_merge::main8a_input(record["path"].as_str().unwrap(), current)','crate::source_reflows_merge::main8a_historical::input(\n        crate::source_reflows_merge::main8a_historical::Domain::Compact7,\n        record["path"].as_str().unwrap(), current,\n    )')]:
 before=git(BASE,path).decode();assert(R/path).read_text()==before.replace(old,new,1);assert(D/'before'/(path.replace('/','__')+'.txt')).read_text()==before
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:assert 'src/source_reflows_merge/main8a_historical.rs' in(R/path).read_text()
assert len(helper.read_text().splitlines())<=1500
emit(D/'serial-chain-audit.json',audit);emit(D/'source-proof.json',dict(status='PASS_SOURCE_ONLY_19_COMPLETE_HISTORICAL_INPUTS',rows=proof,old_serial_chain_mismatches=sum(not r['old_serial_chain_matches']for r in audit),raw_current_audit_reads_remain_raw=True,original_12_and_7_tables_unchanged=True,rust_executed=False))
print(json.dumps(dict(status='PASS_SOURCE_ONLY',historical_rows=19,unique_sources=len(allpaths),old_serial_chain_mismatches=sum(not r['old_serial_chain_matches']for r in audit),write_endpoints=a.write_endpoints,rust_executed=False)))
