from pathlib import Path
import hashlib,json,subprocess,difflib
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
h=lambda b:hashlib.sha256(b).hexdigest()
B={'6444':'6444d6d950bc01be03720c33ad6cb175e62e8cc5','main':'f4a448da6ed71da57719ea7bc8062b134484e8a7'}
for name,rev in B.items():
 rows=[]
 for p in subprocess.check_output(['git','diff','--name-only',rev],cwd=R,text=True).splitlines():
  if not subprocess.check_output(['git','ls-tree',rev,'--',p],cwd=R).strip():continue
  if name=='main' and p.startswith('docs/evidence/'):continue
  old=subprocess.check_output(['git','show',rev+':'+p],cwd=R);new=(R/p).read_bytes()
  if new==old:continue
  # Complete fixed-source rows, no old metadata mutation.
  before=D/'before'/name/p;before.parent.mkdir(parents=True,exist_ok=True);before.write_bytes(old)
  edits=[]
  for context in range(2,33):
   candidate=[]
   for group in difflib.SequenceMatcher(None,new.decode().splitlines(True),old.decode().splitlines(True),autojunk=False).get_grouped_opcodes(context):
    a,b=group[0][1],group[-1][2];c,d=group[0][3],group[-1][4];n=''.join(new.decode().splitlines(True)[a:b]);o=''.join(old.decode().splitlines(True)[c:d]);candidate.append({'new':n,'old':o})
   restored=new
   for e in candidate:
    if not e['new'] or restored.count(e['new'].encode())!=1:break
    restored=restored.replace(e['new'].encode(),e['old'].encode(),1)
   else:
    if restored==old:edits=candidate;break
  if not edits:edits=[{'new':new.decode(),'old':old.decode()}]
  rows.append({'source':p,'checkpoint_revision':rev,'archive':str(before.relative_to(R)),'current_sha256':h(new),'original_sha256':h(old),'edits':edits})
 (D/('current-to-'+name+'.json')).write_text(json.dumps(rows,indent=2)+'\n');print(name,len(rows))
(D/'source-inverses.json').write_bytes((D/'current-to-6444.json').read_bytes())
