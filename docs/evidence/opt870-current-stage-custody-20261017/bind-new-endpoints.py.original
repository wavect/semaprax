#!/usr/bin/env python3
"""Bind only NEW unqualified outer raw endpoints; fixed99/DB2 contracts never change."""
import argparse, difflib, hashlib, json, os, re, subprocess
from pathlib import Path, PurePosixPath
import posixpath
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);ap.add_argument('--source-repo');ap.add_argument('--expected-head',required=True);ap.add_argument('--write',action='store_true');args=ap.parse_args()
assert os.environ.get('TMPDIR')=='/Volumes/Semaprax/runtime-tmp'
R=Path(args.repo).resolve();S=Path(args.source_repo).resolve()if args.source_repo else R;assert R.is_relative_to('/Volumes/Semaprax')and S.is_relative_to('/Volumes/Semaprax')
E=R/'docs/evidence/final-ready-outer-joins-20261017';INDEX_SHA='2deb3dad2f22dd89f62610637943219100599b214f0ddae33d9737f4c8e14091'
sha=lambda b:hashlib.sha256(b).hexdigest()
def git(*a):return subprocess.check_output(['git',*a],cwd=S)
assert git('rev-parse','HEAD').decode().strip()==args.expected_head
index_bytes=(E/'projected-index.json').read_bytes();assert sha(index_bytes)==INDEX_SHA
I=json.loads(index_bytes)
for path,digest in I['protected_originals'].items():
 for repo in {R,S}:assert sha((repo/path).read_bytes())==digest,('immutable original changed',repo,path)
def fixed(key):
 row=I[key];b=(E/row['projected_file']).read_bytes();assert len(b)==row['length']and sha(b)==row['sha256'];return b
P={key:fixed(key)for key in ['owner99','canonical_db2','canonical_831','projection_db2']}
# Independent complete831 source route: original row and full old archive remain fixed.
rec=json.loads((E/'projected/831-record.json').read_text());old_record=next(v for v in json.loads((R/'docs/evidence/opt831-cold-terminal-empty-memo-20261017/predecessor-views.json').read_text())['rows']if v['path']==I['canonical_831']['path']);assert rec==old_record
assert P['canonical_db2']==(R/'docs/evidence/opt831-cold-terminal-empty-memo-20261017'/rec['archive']).read_bytes()
def lines(b):
 text=b.decode();result=text.splitlines(keepends=True);by_lf=[v+'\n'for v in text.split('\n')[:-1]]
 if text.split('\n')[-1]:by_lf.append(text.split('\n')[-1])
 assert result==by_lf,'source line protocol is LF-based';return result
x=lines(P['canonical_831'])
for edit in rec['edits'][::-1]:
 a,z=edit['current_start'],edit['current_end'];assert ''.join(x[a:z])==edit['current'];x[a:z]=lines(edit['original'].encode())
assert ''.join(x).encode()==P['canonical_db2']
# The fixed owner99 projected bytes must be the ORIGINAL admitted99 input, not incoming7f8.
row=next(v for v in json.loads((R/'docs/evidence/main7f8-source-fronts-20261017/inputs.json').read_text())if v['domain']=='source-followups'and v['path']==I['owner99']['path']);assert row['current'].encode()==P['owner99']and row['current_sha256']==sha(P['owner99'])
bridge_rows=json.loads((R/'docs/evidence/current344-input-composition-20261016/bridges.json').read_text())
for key,source in [('canonical_db2',I['canonical_db2']['path']),('projection_db2',I['projection_db2']['source'])]:assert next(v['current_sha256']for v in bridge_rows if v['source']==source)==sha(P[key])
# Parse only the fixed ordered include/literal vocabulary, never discover children.
macro=I['projection_db2'];macro_bytes=(S/macro['macro_source']).read_bytes();text=macro_bytes.decode();start=text.index('macro_rules! '+macro['macro_name']+' {');end=text.index('\nmacro_rules!',start+1);body=text[start:end]
tokens=[];current_chunks=[];bindings={}
for m in re.finditer(r'include_str!\(\s*"([^"\n]+)"\s*\)|("(?:\\.|[^"\\])*")',body):
 if m.group(1):
  path=posixpath.normpath(str(PurePosixPath('tests/workspace').joinpath(m.group(1))));b=(S/path).read_bytes();tokens.append({'path':path});bindings[path]=sha(b);current_chunks.append(b)
 else:
  literal=json.loads(m.group(2));tokens.append({'literal':literal});current_chunks.append(literal.encode())
expected=[{'path':v['path']}if'path'in v else{'literal':v['literal']}for v in macro['tokens']];assert tokens==expected;assert len(tokens)==45
# Authenticate every old projected chunk and its exact separator from the fixed535292-byte whole.
cursor=0
for token in macro['tokens']:
 if'path'in token:
  b=P['projection_db2'][cursor:cursor+token['length']];assert sha(b)==token['sha256'];cursor+=token['length']
 else:
  b=token['literal'].encode();assert P['projection_db2'][cursor:cursor+len(b)]==b;cursor+=len(b)
assert cursor==len(P['projection_db2'])
def inverse(before,after):
 old,current=lines(before),lines(after);edits=[]
 for tag,i,j,a,z in difflib.SequenceMatcher(None,old,current,autojunk=False).get_opcodes():
  if tag!='equal':edits.append(dict(start=a,end=z,current=''.join(current[a:z]),previous=''.join(old[i:j])))
 assert len(edits)<=512
 restored=current[:]
 for edit in edits[::-1]:
  a,z=edit['start'],edit['end'];assert ''.join(restored[a:z])==edit['current'];restored[a:z]=lines(edit['previous'].encode())
 assert ''.join(restored).encode()==before
 return edits
current_owner=(S/I['owner99']['path']).read_bytes();current_canonical=(S/I['canonical_db2']['path']).read_bytes();current_projection=b''.join(current_chunks)
rows=[]
for domain,source,current,target in [('owner99',I['owner99']['path'],current_owner,P['owner99']),('canonical344',I['canonical_db2']['path'],current_canonical,P['canonical_831']),('projection344',I['projection_db2']['source'],current_projection,P['projection_db2'])]:
 assert len(current)<=1048576;rows.append(dict(domain=domain,source=source,current_sha256=sha(current),current=current.decode(),immediate_projected_sha256=sha(target),inverse=inverse(target,current)))
# Complete542 whole optimized_tests inverse, separate from the extracted sequence function.
cut=json.loads((R/'docs/evidence/opt858-observer-followups-20261017/current-to-before.json').read_text());current542=(E/I['whole542']['current_file']).read_bytes();previous542=(E/I['whole542']['previous_file']).read_bytes();assert len(current542)==I['whole542']['current_length'];assert len(previous542)==I['whole542']['previous_length'];assert sha(current542)==cut['current_sha256']==I['whole542']['current_sha256'];assert sha(previous542)==cut['before_sha256']==I['whole542']['previous_sha256'];x=lines(current542)
for edit in cut['inverse'][::-1]:
 a,z=edit['start_line'],edit['end_line'];assert ''.join(x[a:z])==edit['current'];x[a:z]=lines(edit['previous'].encode())
old542=''.join(x).encode();assert sha(old542)==cut['before_sha256'];assert old542==previous542
# Source-bound start/end fingerprint: no concurrent mutation of any consumed raw subject.
bindings.update({I['owner99']['path']:sha(current_owner),I['canonical_db2']['path']:sha(current_canonical),macro['macro_source']:sha(macro_bytes)})
for path,digest in bindings.items():assert sha((S/path).read_bytes())==digest,('raw source raced',path)
assert git('rev-parse','HEAD').decode().strip()==args.expected_head
payload=(json.dumps(rows,indent=2)+'\n').encode();assert len(payload)<=2097152
helper=R/'src/source_reflows_merge/final_ready_inputs.rs';code=helper.read_text();code,n=re.subn(r'(const ENDPOINT_LENGTH: usize = )\d+(;)',lambda m:m[1]+str(len(payload))+m[2],code);assert n==1
code,n=re.subn(r'(const ENDPOINT_SHA256: &str =\s*")[0-9a-f]{64}(";)',lambda m:m[1]+sha(payload)+m[2],code);assert n==1
receipt=dict(status='PASS_NEW_UNQUALIFIED_SOURCE_ENDPOINTS_ONLY',source_head=args.expected_head,source_repo=str(S),output_repo=str(R),projected_index_sha256=INDEX_SHA,raw_inputs=bindings,endpoints=[{k:v for k,v in row.items()if k not in ['current','inverse']}for row in rows],endpoint_length=len(payload),endpoint_sha256=sha(payload),projected_originals_unchanged=True,canonical_831_complete_inverse='PASS',whole542_span_inverse='PASS27_ZERO_BASED_HALF_OPEN',whole542_current=sha(current542),whole542_before=sha(old542),whole542_actual_raw=sha((S/cut['path']).read_bytes()),whole542_needs_distinct_prefix=sha((S/cut['path']).read_bytes())!=sha(current542),qualification='NO_CARGO_OR_RUNTIME',refresh_required_after_all_production_Rust_writes=True)
if args.write:
 (E/'endpoints.json').write_bytes(payload);helper.write_text(code);(E/'endpoint-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
else:assert(E/'endpoints.json').read_bytes()==payload;assert helper.read_text()==code
for path,digest in I['protected_originals'].items():assert sha((R/path).read_bytes())==digest
print(json.dumps(receipt,indent=2))
