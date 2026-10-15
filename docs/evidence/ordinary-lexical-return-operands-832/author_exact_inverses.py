from pathlib import Path
import subprocess,json,hashlib,difflib
r=Path.cwd()
packet=r/'docs/evidence/ordinary-lexical-return-operands-832';(packet/'original').mkdir(parents=True,exist_ok=True)
paths=json.loads(Path(__file__).with_name('source-paths.json').read_text())
def oldtext(path):
 result=subprocess.run(['git','show','d7ec339562b90d655340e315e3ab674ef4d24b12:'+path],cwd=r,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 return result.stdout.decode() if result.returncode==0 else ''
def edits(old,new):
 if old==new:return []
 if not old:return [{'new':new,'before':''}]
 a=old.splitlines(True);b=new.splitlines(True);groups=list(difflib.SequenceMatcher(None,a,b,autojunk=False).get_grouped_opcodes(3));out=[];current=new
 for group in reversed(groups):
  x=''.join(b[group[0][3]:group[-1][4]]);y=''.join(a[group[0][1]:group[-1][2]])
  if not x or current.count(x)!=1:return [{'new':new,'before':old}]
  current=current.replace(x,y,1);out.append({'new':x,'before':y})
 assert current==old
 return out
rows=[];deltas={}
for name in paths:
 current=(r/name).read_text();old=oldtext(name);delta=edits(old,current);deltas[name]=delta
 archived=packet/'original'/ (name.replace('/','__')+'.before.txt');archived.write_text(old)
 rows.append({'source':name,'basis':'d7ec339562b90d655340e315e3ab674ef4d24b12','current_sha256':hashlib.sha256(current.encode()).hexdigest(),'original_sha256':hashlib.sha256(old.encode()).hexdigest(),'original_bytes':old,'edits':delta})
(packet/'exact-source-inverses.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n')
checked=0
for row in rows:
 current=(r/row['source']).read_text()
 for edit in row['edits']:
  assert current.count(edit['new'])==1
  current=current.replace(edit['new'],edit['before'],1)
 assert current==row['original_bytes']
 checked+=1
proof={'status':'SOURCE_ONLY_NO_RUST_EXECUTION','basis':'d7ec339562b90d655340e315e3ab674ef4d24b12','whole_original_rows_reconstructed':checked,'runtime':'UNEXECUTED','rust_compilation':'UNEXECUTED','rust_tests':'UNEXECUTED','whole_832':'OPEN'}
(packet/'source-only-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
print(json.dumps(proof))
