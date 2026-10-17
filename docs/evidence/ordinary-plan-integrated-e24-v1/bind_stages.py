"""Finite e24/PLAN stage binder; retains all old packets byte-for-byte."""
from pathlib import Path
import hashlib,json,difflib,subprocess,re
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
E24='e24e99787e8dec6cf5aed2930ff909cedc187f97';LEGACY='3af65085e'
h=lambda text:hashlib.sha256(text.encode()).hexdigest()
def git(commit,path):return subprocess.check_output(['git','show',commit+':'+path],cwd=R).decode()
def inverse(current,before):
 a=current.splitlines(True);b=before.splitlines(True);offsets=[0]
 for line in a:offsets.append(offsets[-1]+len(line.encode()))
 edits=[]
 for tag,i,j,k,l in difflib.SequenceMatcher(a=a,b=b,autojunk=False).get_opcodes():
  if tag=='equal':continue
  now=''.join(a[i:j]);old=''.join(b[k:l]);prefix=0
  while prefix<min(len(now),len(old)) and now[prefix]==old[prefix]:prefix+=1
  suffix=0
  while suffix<min(len(now)-prefix,len(old)-prefix) and now[-1-suffix]==old[-1-suffix]:suffix+=1
  end_now=len(now)-suffix if suffix else len(now);end_old=len(old)-suffix if suffix else len(old)
  edits.append({'start_byte':offsets[i]+len(now[:prefix].encode()),'after':now[prefix:end_now],'before':old[prefix:end_old]})
 edits.reverse();restored=current.encode();last=len(restored)
 for edit in edits:
  start=edit['start_byte'];after=edit['after'].encode();end=start+len(after)
  assert end<=last and restored[start:end]==after
  restored=restored[:start]+edit['before'].encode()+restored[end:];last=start
 assert restored.decode()==before
 return {'after_sha256':h(current),'before_sha256':h(before),'inverse':edits}
legacy_packet=R/'docs/evidence/ordinary-plan-history-c8740045-v1/inputs.json'
assert legacy_packet.read_text()==git(E24,str(legacy_packet.relative_to(R)))
rows=[]
for old in json.loads(legacy_packet.read_text()):
 path=old['path'];current=(R/path).read_text();checkpoint=git(E24,path);legacy=git(LEGACY,path)
 assert h(legacy)==old['after_sha256']
 rows.append({'path':path,'expected_sha256':old['before_sha256'],'current_length':len(current.encode()),'current_sha256':h(current),
  'e24':checkpoint,'legacy':legacy,'current_to_e24':inverse(current,checkpoint),'e24_to_legacy':inverse(checkpoint,legacy)})
raw=json.dumps(rows,indent=2)+'\n';assert len(raw.encode())<=2*1024*1024
(D/'stages.json').write_text(raw)
p=R/'src/source_reflows_merge/ordinary_plan_inputs.rs';text=p.read_text();text=re.sub(r'const INTEGRATED_LENGTH: usize = \d+;',f'const INTEGRATED_LENGTH: usize = {len(raw.encode())};',text);text=re.sub(r'const INTEGRATED_SHA256: &str = "[^"]*";',f'const INTEGRATED_SHA256: &str = "{h(raw)}";',text);p.write_text(text)
manifest=json.loads((R/'docs/evidence/ordinary-plan-history-c8740045-v1/manifest.json').read_text())
for row in manifest['files']:
 text=(R/row['path']).read_text();row['sha256']=h(text);row['lines']=len(text.splitlines())
manifest['base']=E24;manifest['runtime_verification']='unexecuted; source-only finite composition'
(D/'current-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
# Preserve whole prior code of every changed Rust/source recipe endpoint.
for path in ['src/source_reflows_merge/ordinary_plan_inputs.rs','src/source_reflows_merge/main67107_inputs.rs','docs/evidence/ordinary-plan-history-c8740045-v1/bind_sources.py']:
 target=D/'before'/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(git(E24,path))
print('finite stages',len(rows),'packet bytes',len(raw.encode()))
