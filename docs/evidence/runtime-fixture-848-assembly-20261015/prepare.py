from pathlib import Path
import difflib,hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;H='999fae8f3b82f1ff68408849b4684ea1dd49a1f9';B='8c974ceb05e0bbd63fda796054b00f68f90b5938';g=lambda *a:subprocess.check_output(['git',*a],cwd=R);h=lambda b:hashlib.sha256(b).hexdigest()
def row(p,rev,domain):
 old=g('show',rev+':'+p);new=(R/p).read_bytes();before=D/'before'/domain/p;before.parent.mkdir(parents=True,exist_ok=True);before.write_bytes(old);edits=[]
 if new!=old:
  a=new.decode().splitlines(True);b=old.decode().splitlines(True)
  for context in range(2,33):
   candidate=[{'new':''.join(a[x[0][1]:x[-1][2]]),'old':''.join(b[x[0][3]:x[-1][4]])} for x in difflib.SequenceMatcher(None,a,b,autojunk=False).get_grouped_opcodes(context)];restored=new
   for e in candidate:
    if not e['new'] or restored.count(e['new'].encode())!=1:break
    restored=restored.replace(e['new'].encode(),e['old'].encode(),1)
   else:
    if restored==old:edits=candidate;break
  if not edits:edits=[{'new':new.decode(),'old':old.decode()}]
 return {'source':p,'checkpoint_revision':rev,'archive':str(before.relative_to(R)),'current_sha256':h(new),'original_sha256':h(old),'edits':edits}
changes=[]
for p in g('diff','--name-only',H).decode().splitlines():
 if g('ls-tree',H,'--',p).strip() and (R/p).read_bytes()!=g('show',H+':'+p):changes.append(row(p,H,'999fa'))
assert len(changes)==2
(D/'current-to-999fa.json').write_text(json.dumps(changes,indent=2)+'\n');(D/'source-inverses.json').write_bytes((D/'current-to-999fa.json').read_bytes())
owned=['src/cleanup_plan/replay/selected_context/child/tests/source/compressed_fixture.rs','src/source_reflows_merge.rs'];direct=[row(p,B,'848') for p in owned];(D/'current-to-848.json').write_text(json.dumps(direct,indent=2)+'\n')
tracked=set(g('ls-tree','-r','--name-only',B).decode().splitlines());full=[]
for p in g('diff','--name-only',B).decode().splitlines():
 if p in tracked and (R/p).read_bytes()!=g('show',B+':'+p):full.append(row(p,B,'848'))
(D/'complete-848-inputs.json').write_text(json.dumps(full,indent=2)+'\n');print(json.dumps({'inherited_rows':len(changes),'direct848_rows':len(direct),'complete848_namespace_rows':len(full)}))
