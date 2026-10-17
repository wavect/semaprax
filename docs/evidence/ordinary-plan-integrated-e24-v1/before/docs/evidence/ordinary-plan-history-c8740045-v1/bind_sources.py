from pathlib import Path
import hashlib,json,subprocess,difflib
r=Path(__file__).resolve().parents[3];d=Path(__file__).resolve().parent
h=lambda s:hashlib.sha256(s.encode()).hexdigest()
paths=['src/cleanup_plan/replay.rs','src/source_reflows_merge.rs','src/source_reflows_merge/main67107_inputs.rs','tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
rows=[]
for path in paths:
 old=subprocess.check_output(['git','show','c8740045e33126a4375aadd1dc524a25be9245fc:'+path],cwd=r).decode();new=(r/path).read_text()
 a=old.splitlines(keepends=True);b=new.splitlines(keepends=True);edits=[]
 for tag, i,j,k,l in difflib.SequenceMatcher(a=a,b=b,autojunk=False).get_opcodes():
  if tag!='equal':
   # Include unique equal context, so deletions and one-line additions invert unambiguously.
   lo=max(0,k-2);hi=min(len(b),l+2);alo=max(0,i-2);ahi=min(len(a),j+2)
   edits.append({'after':''.join(b[lo:hi]),'before':''.join(a[alo:ahi])})
 restored=new
 for edit in reversed(edits):
  assert restored.count(edit['after'])==1,(path,edit)
  restored=restored.replace(edit['after'],edit['before'],1)
 assert restored==old,path
 rows.append({'path':path,'before_sha256':h(old),'after_sha256':h(new),'before':old,'inverse':list(reversed(edits))})
(d/'inputs.json').write_text(json.dumps(rows,indent=2)+'\n')
root_row=rows[0]
(d/'root-inverse.json').write_text(json.dumps({key:root_row[key] for key in ['path','before_sha256','after_sha256','inverse']},indent=2)+'\n')
import re
for source,packet in [('src/source_reflows_merge/ordinary_plan_inputs.rs','inputs.json'),('src/cleanup_plan/replay/ordinary_plan_history/source_custody.rs','root-inverse.json')]:
 p=r/source; content=p.read_text(); packet_bytes=(d/packet).read_bytes()
 assert len(packet_bytes)<=2*1024*1024
 content=re.sub(r'const PACKET_LENGTH: usize = \d+;', 'const PACKET_LENGTH: usize = '+str(len(packet_bytes))+';',content)
 content=re.sub(r'const PACKET_SHA256: &str = \"[^\"]*\";', 'const PACKET_SHA256: &str = \"'+hashlib.sha256(packet_bytes).hexdigest()+'\";',content)
 p.write_text(content)

manifest=[]
for p in sorted((r/'src/cleanup_plan/replay/ordinary_plan_history').glob('*.rs')):
 manifest.append({'path':str(p.relative_to(r)),'sha256':h(p.read_text()),'lines':len(p.read_text().splitlines())})
p=r/'src/cleanup_plan/replay/ordinary_plan_history.rs';manifest.append({'path':str(p.relative_to(r)),'sha256':h(p.read_text()),'lines':len(p.read_text().splitlines())})
p=r/'src/source_reflows_merge/ordinary_plan_inputs.rs';manifest.append({'path':str(p.relative_to(r)),'sha256':h(p.read_text()),'lines':len(p.read_text().splitlines())})
(d/'manifest.json').write_text(json.dumps({'base':'c8740045e33126a4375aadd1dc524a25be9245fc','runtime_verification':'not run; integrated parent owns Cargo and runtime','files':manifest},indent=2)+'\n')
print('Bound whole original/inverse rows:',len(rows))
