"""Finite read-only mirror of this source helper's pure string projections.
No cache; every complete JSON input and output SHA is authenticated.
"""
import json,re,hashlib
from pathlib import Path

def split(text,separator=','):
 out=[];start=0;stack=[];quote=False;escaped=False
 for i,c in enumerate(text):
  if quote:
   if escaped:escaped=False
   elif c=='\\':escaped=True
   elif c=='"':quote=False
   continue
  if c=='"':quote=True
  elif c in '([{':stack.append(c)
  elif c in ')]}':stack.pop()
  elif c==separator and not stack:out.append(text[start:i].strip());start=i+1
 out.append(text[start:].strip());return [x for x in out if x]
class Inputs:
 def __init__(self,root):
  self.root=root;self.code=(root/'src/source_reflows_merge.rs').read_text();self.functions={}
  # Helpers contain no braces in literal payloads: only path/message strings.
  for m in re.finditer(r'fn (\w+)\(([^)]*)\)\s*->\s*String\s*\{',self.code):
   start=m.end();depth=1;i=start;quote=False;escape=False
   while depth:
    c=self.code[i]
    if quote:
     if escape:escape=False
     elif c=='\\':escape=True
     elif c=='"':quote=False
    elif c=='"':quote=True
    elif c=='{':depth+=1
    elif c=='}':depth-=1
    i+=1
   self.functions[m[1]]=([x.split(':')[0].strip() for x in m[2].split(',')],self.code[start:i-1])
 def project(self,rows,path,current):
  rows=json.loads(rows) if isinstance(rows,str) else rows;matches=[x for x in rows if x.get('source')==path];assert len(matches)<=1,path
  if not matches:return current
  row=matches[0];assert hashlib.sha256(current.encode()).hexdigest()==row['current_sha256'],path
  for e in row['edits']:
   assert current.count(e['new'])==1,path;current=current.replace(e['new'],e['old'],1)
  assert hashlib.sha256(current.encode()).hexdigest()==row['original_sha256'];return current
 def expr(self,text,env):
  text=text.strip().removeprefix('&').strip()
  if text in env:return env[text]
  if text.endswith('.as_str()') or text.endswith('.to_owned()'):return env[text.split('.')[0]]
  if text.startswith('include_str!'):
   paths=re.findall(r'"(/docs/[^"\n]+)"',text);assert len(paths)==1,text;return (self.root/paths[0].lstrip('/')).read_text()
  if text.startswith('match '):
   key=text.split()[1];value=env[key]
   found=re.search(r'"'+re.escape(value)+r'"\s*=>\s*(include_str!\(concat!\(.*?\)\))',text,re.S);assert found,value
   return self.expr(found[1],env)
  if text.startswith('"'):return json.loads(text)
  m=re.fullmatch(r'(\w+)\((.*)\)',text,re.S);assert m,text
  args=[self.expr(x,env) for x in split(m[2])]
  if m[1]=='project':return self.project(*args)
  return self.call(m[1],*args)
 def call(self,name,*args):
  names,body=self.functions[name];env=dict(zip(names,args));body=re.sub(r'//[^\n]*','',body)
  # Only main's one documented source-specific branch lies on these routes.
  conditional=re.match(r'\s*if source == "([^"]+)" \{\s*return (\w+)\(source, current\);\s*\}',body)
  if conditional:
   if env['source']==conditional[1]:return self.call(conditional[2],env['source'],env['current'])
   body=body[conditional.end():]
  statements=split(body,';')
  for statement in statements[:-1]:
   m=re.fullmatch(r'let (\w+) = (.*)',statement,re.S)
   if m:env[m[1]]=self.expr(m[2],env);continue
   m=re.fullmatch(r'assert_eq!\((.*)\)',statement,re.S);assert m,(name,statement)
   values=split(m[1]);assert self.expr(values[0],env)==self.expr(values[1],env),(name,env['source'])
  return self.expr(statements[-1],env)
 def source36(self,path,current):
  current=self.call('held',path,current)
  rows=json.loads((self.root/'docs/evidence/held-auth-839/source-inverses.json').read_text());matches=[x for x in rows if x['path']==path];assert len(matches)<=1
  if matches:
   row=matches[0];assert current==row['after'];assert hashlib.sha256(current.encode()).hexdigest()==row['after_sha256'];current=row['before'];assert hashlib.sha256(current.encode()).hexdigest()==row['before_sha256']
  current=self.call('ours',path,current)
  return self.project((self.root/'docs/evidence/source36-after-next-838-839-20261015/source-projections.json').read_text(),path,current)
