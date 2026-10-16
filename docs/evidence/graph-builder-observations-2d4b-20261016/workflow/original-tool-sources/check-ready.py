#!/usr/bin/env python3
"""Read-only completed-build/artifact admission; never builds or captures."""
from pathlib import Path
import argparse,hashlib,json,subprocess,tomllib
p=argparse.ArgumentParser();p.add_argument('--round',required=True,type=Path);p.add_argument('--expected-build-receipt-sha256',required=True);p.add_argument('--out',required=True,type=Path);a=p.parse_args();r=a.round.resolve();d=Path(__file__).resolve().parent;record=json.loads((d/'skeleton-record.json').read_text());root=Path(record['repository']);head=record['frozen_source_head']
def sha(p):
 with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
assert not a.out.exists() and not a.out.resolve().is_relative_to(root)
assert r==Path(record['required_round']).resolve(), 'exact76 round required'
assert sha(r/'expected-counts.json')==record['required_manifest_sha256'], 'exact76 manifest required'
assert len(a.expected_build_receipt_sha256)==64 and sha(r/'owning-build-result.json')==a.expected_build_receipt_sha256
m=json.loads((r/'expected-counts.json').read_text());b=json.loads((r/'owning-build-result.json').read_text())
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==head
assert not subprocess.check_output(['git','status','--porcelain','--untracked-files=all'],cwd=root).strip()
assert m['source_subject']==b['source_subject']==head and m['source_tree']==b['source_tree']==record['frozen_source_tree']
assert b['exit_code']==0 and not b.get('artifact_refusals') and b['results'] and all(v['exit_code']==0 for v in b['results'])
assert len(record['required_ordered_build_commands'])==4
assert b['ordered_commands']==record['required_ordered_build_commands'], 'all FOUR exact source-bound build commands must complete in order'
assert len(b['ordered_commands'])==len(b['results']) and [v['command'] for v in b['results']]==b['ordered_commands']
assert not b.get('resource_refusal') and b.get('source_binding_status','unchanged')=='unchanged'
assert b['manifest_sha256']==sha(r/'expected-counts.json') and b['owning_build_metadata_sha256']==sha(r/'owning-build.jsonl')
for n,h in record['trusted_tool_sha256'].items():assert sha(d/n)==h
for n,h in record['old_source_sha256'].items():assert sha(root/n)==h and (root/n).read_bytes()==(d/'frozen-source-before'/n).read_bytes()
for n,h in record['old_actual_all10_wire_sha256'].items():assert sha(Path(record['old_actual_capture'])/'wires'/n)==h
built={str(Path(v['executable']).resolve()):v['executable_sha256'] for v in b['artifacts']};found={};cli={}
with (r/'owning-build.jsonl').open() as f:
 for line in f:
  try:v=json.loads(line)
  except ValueError:continue
  if v.get('reason')!='compiler-artifact' or not v.get('executable'):continue
  package=tomllib.loads(Path(v['manifest_path']).read_text())['package']['name'];name=v['target']['name'];kind=v['target']['kind'];istest=v.get('profile',{}).get('test');exe=Path(v['executable']).resolve()
  role=None
  if package=='semaprax' and istest and ((name=='semaprax' and kind==['lib']) or (name=='workspace' and kind==['test'])):role=name
  binrole=None
  if not istest and kind==['bin'] and ((package=='semaprax' and name=='semaprax') or (package=='semaprax-toolchain' and name=='semaprax-full')):binrole=name
  if role or binrole:
   assert exe.is_relative_to((root/m['environment']['private_target']).resolve()) and exe.is_file();h=sha(exe)
   row={'package':package,'target':v['target'],'profile_test':istest,'executable':str(exe),'executable_sha256':h,'source_head':head,'raw_Cargo_log_sha256':b['owning_build_metadata_sha256'],'completed_build_receipt_sha256':a.expected_build_receipt_sha256}
   dest=found if role else cli;key=role or binrole
   if role:assert built[str(exe)]==h
   if key in dest:assert dest[key]==row
   dest[key]=row
assert set(found)=={'semaprax','workspace'},'required producer harness artifacts absent'
assert set(cli)=={'semaprax','semaprax-full'},'actual completed raw Cargo JSON must identify both production CLI artifacts; no stale fallback'
proof={'status':'ACTUAL_COMPLETED_BUILD_ARTIFACTS_VALIDATED_NO_CAPTURE','source_subject':head,'actual_build_receipt_sha256':a.expected_build_receipt_sha256,'manifest_sha256':sha(r/'expected-counts.json'),'raw_Cargo_log_sha256':sha(r/'owning-build.jsonl'),'producer_harnesses':found,'production_CLI_artifacts_from_same_completed_build':cli,'capture_performed':False}
a.out.write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
