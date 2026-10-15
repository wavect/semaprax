#!/usr/bin/env python3
"""Re-execute immutable prior joined checks at their authenticated source stage.
Current physical→prior joins are checked separately by refresh.py; this runner
labels the inherited stage explicitly and never writes historical proof files.
"""
from pathlib import Path
import subprocess,sys,runpy,io,json
R=Path.cwd();D=Path(__file__).resolve().parent;P=R/'docs/evidence/incremental-source-composition-5f1fecd-20261015';PRIOR='e4b90221df31aa994641d4748d539ec16ea814cb'
changed=set(subprocess.check_output(['git','diff','--name-only',PRIOR],text=True).splitlines())
readbytes=Path.read_bytes;readtext=Path.read_text;openfile=Path.open;writetext=Path.write_text

def earlier(p):
 try:name=str(p.resolve().relative_to(R))
 except ValueError:return None
 if name not in changed:return None
 result=subprocess.run(['git','show',PRIOR+':'+name],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 return result.stdout if result.returncode==0 else None

def rb(p):
 b=earlier(p);return readbytes(p) if b is None else b

def rt(p,*args,**kw):return rb(p).decode(kw.get('encoding') or (args[0] if args else 'utf-8'))
def op(p,mode='r',*args,**kw):
 if mode in ('r','rb'):
  b=earlier(p)
  if b is not None:return io.BytesIO(b) if mode=='rb' else io.StringIO(b.decode(kw.get('encoding') or 'utf-8'))
 return openfile(p,mode,*args,**kw)
def wt(p,data,*args,**kw):
 assert p==P/'joined-source-proof.json','no historical writes'
 return writetext(D/'inherited-joined-recheck.json',data,*args,**kw)
check_output=subprocess.check_output
def output(command,*args,**kw):
 if command[:3]==['git','diff','--name-only']:
  revision=command[3];actual=check_output(command,cwd=R).decode().splitlines();different=[]
  for name in actual:
   target=subprocess.run(['git','show',revision+':'+name],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
   virtual=subprocess.run(['git','show',PRIOR+':'+name],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
   if (target.returncode==0)!=(virtual.returncode==0) or target.returncode==0 and target.stdout!=virtual.stdout:different.append(name)
  result='\n'.join(different)+('\n' if different else '')
  return result if kw.get('text') else result.encode()
 return check_output(command,*args,**kw)
subprocess.check_output=output
Path.read_bytes=rb;Path.read_text=rt;Path.open=op;Path.write_text=wt
sys.path.insert(0,str(P));sys.argv=[str(P/'verify_joined.py'),'--basis','8967ab82fbd11758c1b68e064cfd42f66cfb01c7']
try:runpy.run_path(str(P/'verify_joined.py'),run_name='__main__')
finally:subprocess.check_output=check_output;Path.read_bytes=readbytes;Path.read_text=readtext;Path.open=openfile;Path.write_text=writetext
