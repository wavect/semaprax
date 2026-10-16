#!/opt/homebrew/bin/python3
"""Exact source composition only; never executes Rust or refreshes endpoints."""
from pathlib import Path
import json,hashlib,subprocess,re
E=Path(__file__).resolve().parent;R=E.parents[2];M=json.loads((E/'source-bindings.json').read_text());DATA=(E/'inputs.json').read_bytes();assert hashlib.sha256(DATA).hexdigest()==M['inputs_sha256'];rows=json.loads(DATA)
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(c,p):return subprocess.check_output(['git','show',c+':'+p],cwd=R)
def project(domain,path,source):
 raw=source.encode();matching=[x for x in rows if x['domain']==domain and x['path']==path and x['current_sha256']==sha(raw)]
 assert len(matching)==1
 row=matching[0];out=b'';cursor=0
 for edit in row['edits']:
  start=edit['start'];end=edit['end'];assert cursor<=start<=end<=len(raw);assert raw[start:end]==edit['after'].encode();out+=raw[cursor:start]+edit['before'].encode();cursor=end
 out+=raw[cursor:];assert sha(out)==row['incoming_sha256'];assert out==git(M['incoming'],path)
 return out.decode()
negatives=0
for row in rows:
 source=(R/(row.get('input_archive') or row['path'])).read_text();project(row['domain'],row['path'],source)
 for wrong in ['',source+' ',source+source]:
  try:project(row['domain'],row['path'],wrong)
  except AssertionError:negatives+=1
  else:raise AssertionError('bad complete input accepted')
 try:project('foreign',row['path'],source)
 except AssertionError:negatives+=1
 else:raise AssertionError('foreign domain accepted')
for row in M['immutable_incoming_capsules']:assert sha((R/row['path']).read_bytes())==row['sha256'] and (R/row['path']).read_bytes()==git(M['incoming'],row['path'])
assert sha((R/'src/source_reflows_merge/main_eb2_inputs.rs').read_bytes())==M['helper_sha256']
# Strip ONLY the named additive read registrations; every old assertion/body is exact.
for row in M['before']:
 raw=(R/row['path']).read_bytes();before=(E/row['archive']).read_bytes();assert sha(raw)==row['current_sha256'];assert sha(before)==row['before_sha256'];assert before==git(M['merge'],row['path'])
 path=row['path'];current=raw.decode()
 if path in ['src/cleanup_plan/replay/allocation/tests/clone_cursor.rs','src/wasm/aggregate/semantic_work.rs','src/hir/lexical_exit/walk/tests.rs']:
  current=''.join(line for line in current.splitlines(True) if 'let current = crate::source_reflows_merge::main_eb2_inputs::input(' not in line)
 elif path.endswith('/presence.rs'):
  new='    *current = crate::source_reflows_merge::main_eb2_inputs::presence(path, current);\n';assert current.count(new)==1;current=current.replace(new,'',1)
 elif path=='src/source_reflows_merge.rs':
  new='\npub(crate) mod main_eb2_inputs;\n';assert current.endswith(new);current=current[:-len(new)]
 elif path.endswith('guard-extension-allowed.json'):
  a=json.loads(current);b=json.loads(before);assert a.keys()==b.keys()
  for guard in b:
   for policy in b[guard]:
    expected=dict(b[guard][policy]);expected['src/source_reflows_merge/main_eb2_inputs.rs']=1;assert a[guard][policy]==expected
  continue
 else:
  marker=re.findall(r'include_str!\("[^"\n]+/src/source_reflows_merge/main_eb2_inputs.rs"\), ',current);assert len(marker)==1;current=current.replace(marker[0],'',1)
 assert current.encode()==before,path
# Reproduce every unmodified incoming whole-source assertion against projected inputs.
old_controls=0
for packet in M['packets']:
 evidence=R/'docs/evidence'/packet
 for record in json.loads((evidence/'sources.json').read_text()):
  original=(evidence/record['archive']).read_text();assert sha(original.encode())==record['sha256'];current=project('incoming',record['path'],(R/record['path']).read_text())
  def restore(source):
   for edit in reversed(record['edits']):
    if source.count(edit['new'])!=1:return None
    source=source.replace(edit['new'],edit['old'],1)
   return source
  assert restore(current)==original;assert restore(current+'\n')!=original;old_controls+=1
# The unchanged Main presence inverse runs AFTER its scoped full-input composition.
presence=json.loads((R/'src/cleanup_plan/replay/live_oracle/source_custody/presence.json').read_text())
old_presence=json.loads(git(M['parent'],'src/cleanup_plan/replay/live_oracle/source_custody/presence.json'))
for current_record,prior in zip(presence,old_presence):
 assert current_record['before_sha256']==prior['before_sha256'];assert all(e in current_record['inverse']for e in prior['inverse'])
 for row in [x for x in rows if x['domain']=='presence' and x['path']==current_record['path']]:
  current=project('presence',row['path'],(R/row['input_archive']).read_text());assert sha(current.encode())==current_record['after_sha256']
  for edit in current_record['inverse']:assert current.count(edit['after'])==1;current=current.replace(edit['after'],edit['before'],1)
  assert sha(current.encode())==current_record['before_sha256']
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
print(json.dumps({'status':'PASS_SOURCE_ONLY_MAIN_EB2_COMPOSITION','finite_inputs':len(rows),'whole_input_or_domain_negatives':negatives,'immutable_incoming_capsule_files':len(M['immutable_incoming_capsules']),'incoming_full_original_controls':old_controls,'old_presence_before_sha_and_all_edits_preserved':True,'all9_complete_before_inputs':len(M['before']),'raw_new_helper_in_all3_guards':True,'native_lines':1500,'Rust_execution':False,'final_currentCI_and_overall_endpoint_refresh':'PENDING_PARENT'},sort_keys=True))
