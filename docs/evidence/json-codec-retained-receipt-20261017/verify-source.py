#!/usr/bin/env python3
"""Exact source preservation/registration checks, never executable Rust evidence."""
from pathlib import Path
import hashlib,json,copy,re
E=Path(__file__).resolve().parent;R=E.parents[2]
M=json.loads((E/'source-cuts.json').read_text())
def sha(data):return hashlib.sha256(data).hexdigest()
def restore(row,data):
 assert sha(data)==row['current_sha256']
 lines=data.decode().splitlines(keepends=True)
 for edit in reversed(row['edits']):
  a,z=edit['current_start'],edit['current_end']
  assert ''.join(lines[a:z])==edit['current']
  lines[a:z]=edit['original'].splitlines(keepends=True)
 original=''.join(lines).encode()
 assert sha(original)==row['sha256']
 assert original==(E/row['archive']).read_bytes()
 return original
originals={row['path']:restore(row,(R/row['path']).read_bytes()) for row in M['rows']}
for row in M['new_sources']+M['unchanged_owners']:
 assert sha((R/row['path']).read_bytes())==row['sha256'],row['path']
for row in M['rows']:
 assert len(re.findall(rb'\bassert(?:_eq|_ne)?!',originals[row['path']]))==row['old_assertions']
cli=(R/'src/cli/json_codec.rs').read_text();old=originals['src/cli/json_codec.rs'].decode()
for start,end in [('pub(crate) fn parse(', 'pub(crate) fn dispatch('),('fn run_codec(',None)]:
 a=old[old.index(start):old.index(end) if end else None]
 z=cli[cli.index(start):cli.index(end) if end else None]
 assert a==z
assert (R/'src/project/authority.rs').read_text()=='mod retained_input;\n'+originals['src/project/authority.rs'].decode()
new=(R/'src/project/json_codec/retained_receipt.rs').read_text()
assert new.count('derive_json_codec_plan(&revision, entries)')==1
assert new.count(' = derive()?;')==2
assert 'if first != second || bundle != repeated' in new
assert 'compiler_binary_sha256: None' in new and 'independent_execution: false' in new
assert 'snapshot.with_authenticated_request' in new and 'crate::project::with_authenticated_project' in new
held=(R/'src/project/authority/retained_input.rs').read_text()
assert held.count('self.recheck()?;')==2
assert 'bytes.len() != self.expected_len || observed != self.expected_sha256' in held
for path in ['src/project/json_codec/retained_receipt.rs','src/cli/json_codec/retained_receipt.rs','src/project/authority/retained_input.rs']:
 text=(R/path).read_text()
 assert not any(x in text for x in ['Command::','current_exe','File::open','OpenOptions::','std::fs::read'])
 assert len(text.splitlines())<=1500
# Explicit child paths, particularly when parents themselves are path-loaded.
for path in ['src/project/json_codec/retained_receipt.rs','src/cli/json_codec/retained_receipt.rs']:
 assert '#[path = "retained_receipt/tests.rs"]' in (R/path).read_text()
 assert ((R/path).parent/'retained_receipt/tests.rs').is_file()
assert '#[path = "application_json/retained_receipt.rs"]' in (R/'tests/project/standard_library/application_json.rs').read_text()
for row in M['rows']:
 if row['path'].endswith('.rs'):assert len((R/row['path']).read_text().splitlines())<=1500
controls=0
for row in M['rows']:
 data=(R/row['path']).read_bytes()
 for bad in [data+b' ',data[:-1],data[:5]+b'!'+data[6:]]:
  try:restore(row,bad)
  except AssertionError:controls+=1
  else:raise AssertionError('source drift accepted')
 bad=copy.deepcopy(row);bad['edits'][0]['original']+=' '
 try:restore(bad,data)
 except AssertionError:controls+=1
 else:raise AssertionError('forged inverse accepted')
owners=json.loads((E/'pending-owners.json').read_text())['owners']
for owner in owners:
 if owner['kind']=='bin':assert owner['selector'].startswith('cli_driver::cli::json_codec::')
 assert 'fn '+owner['selector'].split('::')[-1]+'(' in (R/owner['source']).read_text()
print(json.dumps({'status':'PASS_SOURCE_ONLY','complete_originals':len(M['rows']),'new_rust_sources':len(M['new_sources']),'unchanged_owners':len(M['unchanged_owners']),'original_assertions_preserved':sum(row['old_assertions'] for row in M['rows']),'negative_source_controls':controls,'pending_executable_owners':len(owners),'rust_execution':'NOT_RUN','provider_calls':0},indent=2))
