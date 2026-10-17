"""Focused offline package preservation and dispatcher-shape checks."""
from pathlib import Path
import ast,hashlib,json
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest()
bind=json.loads((D/'bindings.json').read_text())
for row in bind['scripts']:
 assert (R/row['path']).read_bytes()==(D/'originals'/row['path']).read_bytes()
 assert sha((R/row['path']).read_bytes())==row['sha256']
for name in ['entry.py','memoizer.py','check-pure-masks.py']:
 ast.parse((D/name).read_text())
print(json.dumps({'status':'ALL_SEVEN_COMPLETE_ORIGINAL_SCRIPT_BYTES_PRESERVED','original_scripts':len(bind['scripts'])}))
# Whole original refresh restoration and literal default commands, no refresh run.
import sys,os,types
p=R/'docs/evidence/joined-batch-custody-20261016/refresh.py';current=p.read_text();tree=ast.parse(current);helper=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='source_command');helper_text=ast.get_source_segment(current,helper)+'\n';restored=current.replace(helper_text,'',1)
restored=restored.replace(";p.add_argument('--memoize-source-masks',action='store_true')",'',1)
for entry,script,args in [
 ('authority-generate',"D/'generate-authority.py'","['--repo',str(R),'--out',str(D)]"),
 ('boundary-generate',"B/'generate-boundary.py'","['--repo',str(R),'--full-dir',str(D),'--out',str(B)]"),
 ('boundary-verify',"B/'verify-boundary.py'","['--repo',str(R),'--full-dir',str(D),'--scope-dir',str(B)]")]:
 new="source_command("+repr(entry)+','+script+','+args+')';old='[sys.executable,str('+script+'),'+args[1:]
 assert restored.count(new)==1;restored=restored.replace(new,old,1)
assert restored==(D/'refresh-before.py').read_text(),'whole original refresh restoration'
ns={'sys':sys,'os':os,'Path':Path,'R':R,'a':types.SimpleNamespace(memoize_source_masks=False,expected_head='not-used')};exec(compile(ast.Module(body=[helper],type_ignores=[]),str(p)+':dispatch','exec'),ns)
for entry in ['authority-generate','boundary-generate','boundary-verify']:
 script=R/(entry+'.py');args=['--repo',str(R),'--out','/Volumes/Semaprax/unused']
 assert ns['source_command'](entry,script,args)==[sys.executable,str(script),*args]
print(json.dumps({'status':'COMPLETE_ORIGINAL_REFRESH_RESTORED_AND_THREE_DEFAULT_COMMANDS_IDENTICAL','refresh_before_sha256':sha((D/'refresh-before.py').read_bytes())}))
