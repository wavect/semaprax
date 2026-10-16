"""Finite source custody only; no Cargo, Rust discovery or behavioral verdict."""
from pathlib import Path
import json,hashlib,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='364d824194cd1df76b5c273aa0fb9cbafb602e29';M='6966765c965c8a5ad85ae3c2827653599826fb88'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=R)
def restore(row,b):
 assert sha(b)==row['current_sha256'],row['source'];s=b.decode()
 for e in row['edits']:
  assert s.count(e['new'])==1,row['source'];s=s.replace(e['new'],e['old'],1)
 assert sha(s.encode())==row['original_sha256'],row['source'];return s.encode()
views={};proof={}
for name,rev in [('364d',B),('main696',M)]:
 rows=json.loads((D/('current-to-'+name+'.json')).read_text());v={}
 for row in rows:
  p=row['source'];v[p]=restore(row,(R/p).read_bytes());assert v[p]==git('show',rev+':'+p)==(R/row['archive']).read_bytes()
 if name=='364d':assert set(v)==set(git('diff',rev,'--name-only','--diff-filter=M').decode().splitlines())
 views[name]=v;proof[name]={'whole_input_rows':len(rows),'compact_bytes':(D/('current-to-'+name+'.json')).stat().st_size}
# Independent Main semantic custody, then unchanged formatting inverses. Never
# send the already restored Main predecessor through the modern raw front again.
semantic=json.loads((R/'docs/evidence/receiver-source-custody-v1/inverse.json').read_text());assert len(semantic)==4
records=json.loads((R/'docs/evidence/mainf045-source-successor-20261015/live-mainf045-ci93.json').read_text());formatting={x['path']:x for x in records}
for row in semantic:
 p=row['path'];b=views['main696'].get(p,(R/p).read_bytes());assert b==git('show',M+':'+p);assert sha(b)==row['after_sha256'];s=b.decode()
 for e in row['inverse']:
  assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
 assert sha(s.encode())==row['before_sha256'];assert s.encode()==(R/'docs/evidence/receiver-source-custody-v1'/row['archive']).read_bytes()
 if p in formatting:
  f=formatting[p];assert sha(s.encode())==f['after_sha256']
  for e in f['inverse']:
   assert s.count(e['after'])==1;s=s.replace(e['after'],e['before'],1)
  assert sha(s.encode())==f['before_sha256']
# Main complete old/current receivers are instantiated without body rewriting.
root=(R/'src/cleanup_plan/replay.rs').read_text();before=(R/'src/cleanup_plan/replay/path_array/before.rs').read_text()
for row in json.loads((R/'docs/evidence/minimum-path-array-v1/functions.json').read_text()):
 name=row['name'];body=row['body'];assert sha(body.encode())==row['sha256'];assert before.count(body.replace('fn '+name+'(','pub(super) fn '+name+'(',1))==1
 if name=='sequence_skeleton_paths':body=body.replace('    let mut combined = Vec::new();\n','    let mut combined = Vec::new();\n    if !suffixes.is_empty() {\n        path_array::minimum(\n            &mut combined,\n            prefixes.len(),\n            work,\n            "sequenced path capacity",\n        )?;\n    }\n',1)
 elif name=='append_expr_paths':body=body.replace('    for path in paths {','    path_array::minimum(target, paths.len(), work, phase)?;\n    for path in paths {',1)
 else:body=body.replace('    let mut results = Vec::new();\n','    let mut results = Vec::new();\n    path_array::minimum(&mut results, paths.len(), work, "'+('status' if name=='split_status_paths' else 'contract')+' path capacity")?;\n',1)
 assert root.count(body)==1,name
for p in ['src/cleanup_plan/replay/path_array.rs','src/cleanup_plan/replay/path_array/before.rs','src/cleanup_plan/replay/path_array/tests.rs']:assert (R/p).read_bytes()==git('show',M+':'+p)
# All active831 owners and the paid inline block remain byte-exact the cutoff;
# only the four declared capacity hooks and root child registration are added.
for p in ['src/cleanup_plan/replay/factored/canonical_shared.rs','src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs','src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs','src/cleanup_plan/replay/branch_clone.rs','src/cleanup_plan/replay/branch_clone/inline_tests.rs']:
 assert (R/p).read_bytes()==git('show',B+':'+p)
oldhelper=git('show',B+':src/source_reflows_merge.rs').decode();helper=(R/'src/source_reflows_merge.rs').read_text();assert helper[helper.index('fn project('):helper.index('// Exact post-build')]==oldhelper[oldhelper.index('fn project('):oldhelper.index('// Exact post-build')]
for p in ['src/source_reflows_merge/runtime_json/inventory.rs','src/source_reflows_merge/runtime_json/membership.rs']:assert (R/p).read_bytes()==git('show',B+':'+p)
assert not git('diff',B,'--name-only','--diff-filter=M','--','docs/evidence/**/*.json').strip()
new=['src/cleanup_plan/replay/path_array.rs','src/cleanup_plan/replay/path_array/before.rs','src/cleanup_plan/replay/path_array/tests.rs','src/source_reflows/semantic_receivers.rs']
for owner in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 for p in new:assert (R/owner).read_text().count('/'+p+'"')==1
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
parser=R/'docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py';ns={};exec(compile(parser.read_text(),str(parser),'exec'),ns);ns['R']=R;original=ns['parse'];loaded=[]
def capture(path,*a,**kw):
 path=Path(path).resolve();assert path.is_file(),str(path);loaded.append(str(path.relative_to(R)));return original(path,*a,**kw)
ns['parse']=capture
capture(R/'src/cleanup_plan/replay/path_array.rs',['cleanup_plan','replay','path_array'],[],'lib','semaprax')
capture(R/'src/source_reflows.rs',['source_reflows'],[],'lib','semaprax')
checks=[{'package':k[0],'kind':k[1],'selector':k[2],**v} for k,v in ns['IDX'].items()];assert len([x for x in checks if x['selector'].startswith('cleanup_plan::replay::path_array::tests::')])==4
assert len([x for x in checks if x['selector'].startswith('source_reflows::semantic_receivers::')])==2
(D/'recursive-module-binding.json').write_text(json.dumps({'mode':'SOURCE_ONLY_NOT_RUST_DISCOVERY','parser_sha256':sha(parser.read_bytes()),'loaded_sources':sorted(set(loaded)),'checks':checks},indent=2)+'\n')
proof.update(status='SOURCE_CUSTODY_ONLY_PASS',Main_semantic_inputs=4,complete_formatting_after_semantic=True,Main_receiver_bodies=4,original_90_runtime_fixtures_exact=True,old_json_unchanged=True,shipping_831_and_inline_owners_exact_cutoff=True,Rust_executed=False,production_funding_review='Separate pending review; this source proof grants no production qualification.')
(D/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
