import re,json,hashlib,subprocess
from pathlib import Path
R=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang'); OUT=Path('/Volumes/Semaprax/lane-prompts/minimum-next-966-draft-20261013')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def mask(s):
 out=list(s);i=0;n=len(s)
 def blank(a,b):
  for z in range(a,b):
   if out[z]!='\n':out[z]=' '
 while i<n:
  start=i
  if s.startswith('//',i):i=s.find('\n',i);i=n if i<0 else i;blank(start,i);continue
  if s.startswith('/*',i):
   i+=2;d=1
   while i<n and d:
    if s.startswith('/*',i):d+=1;i+=2
    elif s.startswith('*/',i):d-=1;i+=2
    else:i+=1
   blank(start,i);continue
  raw=re.compile(r'(?:b|c)?r(#{0,16})"').match(s,i) if s[i] in 'bcr' else None
  if raw:
   end='"'+raw[1];z=s.find(end,i+len(raw[0]));i=n if z<0 else z+len(end);blank(start,i);continue
  if s[i]=='"' or (s[i]=="'" and re.compile(r"'(?:\\.|[^'\\\n])'").match(s,i)):
   q=s[i];i+=1
   while i<n:
    if s[i]=='\\':i+=2
    elif s[i]==q:i+=1;break
    else:i+=1
   blank(start,min(i,n));continue
  i+=1
 return ''.join(out)
# Source only: enumerate registered test declarations, exclude cfg(any()) bodies.
IDX={};seen=set(); snapshots=[]
def parse(p,prefix,chain,kind,package,custom_path=False):
 p=p.resolve();key=(p,tuple(prefix),kind,package,custom_path)
 if key in seen:return
 seen.add(key)
 if not p.exists():return
 s=p.read_text();m=mask(s); db=p.parent if custom_path or p.name in ['lib.rs','main.rs','mod.rs'] else p.parent/p.stem
 def segment(a,b,pre,defaultbase,attrbase,ch):
  i=a;attrs=[]
  while i<b:
   if m[i].isspace():i+=1;continue
   if m.startswith('#[',i):
    j=m.find(']',i)+1
    if not j:break
    attrs.append(s[i:j]);i=j;continue
   md=re.compile(r'(?:pub(?:\([^)]*\))?\s+)?mod\s+(\w+)\s*([;{])').match(m,i)
   if md:
    name=md[1];pos=i;start=i+len(md[0]);disabled=any(re.search(r'cfg\s*\(\s*any\s*\(\s*\)',x) for x in attrs)
    reg={'source_path':str(p.relative_to(R)),'line':s.count('\n',0,pos)+1,'module':name,'attributes':attrs[:]}
    if md[2]=='{':
     d=1;j=start
     while j<b and d:
      d+=(m[j]=='{')-(m[j]=='}');j+=1
     if not disabled:segment(start,j-1,pre+[name],defaultbase/name,defaultbase/name,ch+[reg])
     else:snapshots.append(reg)
     i=j
    else:
     path=next((re.search(r'path\s*=\s*"([^"]+)"',x)[1] for x in attrs if re.search(r'path\s*=\s*"([^"]+)"',x)),None)
     q=attrbase/path if path else defaultbase/(name+'.rs')
     if not path and not q.exists():q=defaultbase/name/'mod.rs'
     if not disabled:parse(q,pre+[name],ch+[reg],kind,package,custom_path=bool(path))
     else:snapshots.append(reg)
     i=start
    attrs=[];continue
   fn=re.compile(r'(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)\s*\(').match(m,i)
   if fn:
    name=fn[1];pos=i
    if any(re.fullmatch(r'#\[\s*test\s*\]',x) for x in attrs):
     sel='::'.join(pre+[name]);IDX[(package,kind,sel)]={'source_path':str(p.relative_to(R)),'line':s.count('\n',0,pos)+1,'name':name,'attributes':attrs[:],'registration_chain':ch[:],'sha256':sha(p),'ignored':any('#[ignore' in x for x in attrs)}
    attrs=[]
   if m[i]=='{':
    d=1;i+=1
    while i<b and d:d+=(m[i]=='{')-(m[i]=='}');i+=1
    attrs=[];continue
   if m[i]==';':attrs=[]
   i+=1
 segment(0,len(s),prefix,db,p.parent,chain)
