"""Finite source-role inventory from the unchanged registration parser; no Rust execution."""
from pathlib import Path
import argparse,json,re,hashlib,tomllib
A=argparse.ArgumentParser();A.add_argument('--repo',type=Path,required=True);A.add_argument('--out',type=Path);a=A.parse_args();R=a.repo.resolve();D=R/'docs/evidence/production-source-authority-partition-20261016';O=a.out or Path(__file__).resolve().parent;O.mkdir(parents=True,exist_ok=True)
sha=lambda b:hashlib.sha256(b).hexdigest();ns={};parser=D/'original-module-parser-prefix.py';exec(compile(parser.read_text(),str(parser),'exec'),ns);ns['R']=R;original=ns['parse'];files={}
def capture(p,prefix,chain,kind,package,custom_path=False):
 p=p.resolve()
 if p.is_file():files.setdefault(str(p.relative_to(R)),[]).append((kind,chain,package,None))
 return original(p,prefix,chain,kind,package,custom_path)
ns['parse']=capture
for p,k in [('src/lib.rs','lib'),('tests/hir_module_boundaries.rs','test:hir_module_boundaries'),('tests/workspace.rs','test:workspace'),('tests/wasm.rs','test:wasm')]:capture(R/p,[],[],k,'semaprax')
# These are registration witnesses only; every production forbidden predicate remains unchanged.
for p,k in [('src/main.rs','bin:semaprax'),('tests/cli_help_surface_v1.rs','test:cli_help_surface_v1'),('tests/project.rs','test:project')]:capture(R/p,[],[],k,'semaprax')
capture(R/'crates/semaprax-toolchain/tests/cli_help_surface_v1.rs',[],[],'test:cli_help_surface_v1','semaprax-toolchain')
mask=ns['mask']
def closed(kind,chain):return kind.startswith('test:') or any(any(re.fullmatch(r'#\[\s*cfg\s*\(\s*test\s*\)\s*\]',x) for x in n['attributes']) for n in chain)
def close(m,i,left,right):
 depth=1;j=i+1
 while depth:
  depth+=(m[j]==left)-(m[j]==right);j+=1
 return j
# Expand only the existing literal inventory macro language, never Rust bodies.
# Normalize only the exact declared test-transport macro for the unchanged
# literal-inventory grammar. The current raw source bytes are never normalized.
JOIN='macro_rules! audit_source_join { ($($piece:expr),* $(,)?) => {{ let pieces: &[&str] = &[$(&$piece),*]; pieces.concat() }}; }'
def inventory_transport(s):
 assert s.startswith(JOIN)
 return s[len(JOIN):].replace('audit_source_join!', 'concat!')
inv=inventory_transport((R/'tests/workspace/semantic_graph/source_inventory.rs').read_text());im=mask(inv);macros={}
for t in re.finditer(r'macro_rules!\s+(\w+)\s*\{',im):
 end=close(im,t.end()-1,'{','}');body=inv[t.end():end-1];bm=mask(body);arms={}
 for v in re.finditer(r'\(([^)]*)\)\s*=>\s*\{',bm):arms[v[1].strip()]=body[v.end():close(bm,v.end()-1,'{','}')-1]
 macros[t[1]]=arms
class Expr:
 def __init__(self,s,base):self.s=s;self.i=0;self.base=base
 def skip(self):
  while True:
   while self.i<len(self.s) and self.s[self.i].isspace():self.i+=1
   if self.s.startswith('//',self.i):self.i=self.s.find('\n',self.i);continue
   if self.s.startswith('/*',self.i):self.i=self.s.index('*/',self.i)+2;continue
   break
 def eat(self,t):self.skip();assert self.s.startswith(t,self.i),(t,self.s[self.i:self.i+70]);self.i+=len(t)
 def string(self):
  self.skip();m=re.match(r'"(?:\\.|[^"\\])*"',self.s[self.i:]);assert m;self.i+=len(m[0]);return json.loads(m[0])
 def expr(self):
  self.skip()
  if self.s[self.i]=='"':return [{'literal':self.string()}]
  m=re.match(r'(\w+)\s*!\s*\(',self.s[self.i:]);assert m,self.s[self.i:self.i+100];self.i+=len(m[0]);n=m[1]
  if n=='include_str':
   path=self.string();self.eat(')');return [{'path':str((self.base/path).resolve().relative_to(R))}]
  if n=='concat':
   out=[]
   while True:
    self.skip()
    if self.s[self.i]==')':self.i+=1;return out
    out+=self.expr();self.skip()
    if self.s[self.i]==',':self.i+=1
  assert n in macros,n;end=self.s.index(')',self.i);key=self.s[self.i:end].strip();self.i=end+1;return Expr(macros[n][key],self.base).expr()
hir=inventory_transport((R/'tests/hir_module_boundaries.rs').read_text());at=hir.index('concat!(',hir.index('let validation_raw'));hir_chunks=Expr(hir[at:],R/'tests').expr()
projection_parts=[('semantic_graph_private_layout_owned_source_inventory',''),('semantic_graph_projection_child_source_inventory','type_rewrite'),('semantic_graph_projection_child_source_inventory','import_stub'),('semantic_graph_projection_child_source_inventory','uncached_peak'),('semantic_graph_projection_root_source_inventory','')]+[('semantic_graph_projection_child_source_inventory',x) for x in ['local_identity','call_identity','call_sites','cost','declaration_cost','identity_slots','statement_segment','defaults']]
projection_chunks=[]
for i,(n,k) in enumerate(projection_parts):
 if i:projection_chunks.append({'literal':'\n'})
 projection_chunks+=Expr(macros[n][k],R/'tests/workspace').expr()
needed={x['path'] for x in hir_chunks+projection_chunks if 'path'in x}
# The existing parser's include! / test-data edges are authenticated literally.
for target in sorted(needed-set(files)):
 candidates=[]
 for parent,registrations in list(files.items()):
  if not all(closed(record[0],record[1]) for record in registrations):continue
  text=(R/parent).read_text()
  if Path(target).name not in text:continue
  masked=mask(text)
  for m in re.finditer(r'\binclude(?:_str)?!\(\s*"([^"]+)"\s*\)',text):
   if not masked.startswith('include',m.start()):continue
   if str(((R/parent).parent/m[1]).resolve().relative_to(R))!=target:continue
   for record in registrations:candidates.append((record[0],record[1],record[2],{'parent':parent,'start':len(text[:m.start()].encode()),'text':m[0],'attributes':[],'child':target}))
 assert candidates,('unclassified inventory source',target)
 files[target]=candidates
roots={('semaprax','lib'):'src/lib.rs',('semaprax','bin:semaprax'):'src/main.rs',('semaprax','test:hir_module_boundaries'):'tests/hir_module_boundaries.rs',('semaprax','test:workspace'):'tests/workspace.rs',('semaprax','test:wasm'):'tests/wasm.rs',('semaprax','test:project'):'tests/project.rs',('semaprax','test:cli_help_surface_v1'):'tests/cli_help_surface_v1.rs',('semaprax-toolchain','test:cli_help_surface_v1'):'crates/semaprax-toolchain/tests/cli_help_surface_v1.rs'}
def manifest(chunks):
 raw_paths={x['path']for x in chunks if 'path'in x}
 # Follow the actual registered module closure, including every production
 # descendant and closed test child absent from the older textual union.
 paths=set(raw_paths)
 for path,records in files.items():
  if any(any(node['source_path']in raw_paths for node in record[1])for record in records):paths.add(path)
 # Literal compiled include! fragments are children too. Unknown/nonliteral
 # include forms fail this finite inventory rather than narrowing the scan.
 pending=list(paths);walked=set()
 while pending:
  parent=pending.pop()
  if parent in walked:continue
  walked.add(parent);text=(R/parent).read_text();masked=mask(text)
  for use in re.finditer(r'\binclude!\s*\(',masked):
   match=re.match(r'include!\s*\(\s*"([^"]+)"\s*\)',text[use.start():]);assert match,('nonliteral compiled include requires explicit inventory',parent)
   target=str(((R/parent).parent/match[1]).resolve().relative_to(R))
   if target not in paths:paths.add(target);pending.append(target)
   if target not in files:
    assert all(record[3] is None for record in files[parent]),'nested include requires explicit inherited registration'
    edge={'parent':parent,'start':len(text[:use.start()].encode()),'text':match[0],'attributes':[],'child':target}
    files[target]=[(record[0],record[1],record[2],edge)for record in files[parent]]
 closure=sorted(paths-raw_paths);classes={};source_paths=set(paths)
 for p in sorted(paths):
  chains=[]
  for record in files[p]:
   kind,chain=record[:2];edges=[]
   for i,node in enumerate(chain):
    parent=node['source_path'];text=(R/parent).read_text();masked=mask(text);line_start=sum(len(x)for x in text.splitlines(True)[:node['line']-1]);m=re.search(r'(?:pub(?:\([^)]*\))?\s+)?mod\s+'+re.escape(node['module'])+r'\s*[;{]',masked[line_start:]);assert m;pos=line_start+m.start();end=line_start+m.end();start=pos
    for attribute in reversed(node['attributes']):start=text.rfind(attribute,0,start);assert start>=0
    child=chain[i+1]['source_path']if i+1<len(chain)else (record[3]['parent']if record[3] is not None else p)
    edges.append({'parent':parent,'start':len(text[:start].encode()),'text':text[start:end],'attributes':node['attributes'],'child':child});source_paths.update([parent,child])
   if record[3] is not None:edges.append(record[3]);source_paths.add(record[3]['parent'])
   root=roots[(record[2],kind)];value={'root':root,'kind':'lib'if kind=='lib'else 'bin' if kind.startswith('bin:') else 'test','package':record[2],'edges':edges};source_paths.add(root)
   if value not in chains:chains.append(value)
  is_test=all(closed(record[0],record[1])for record in files[p]);classes[p]={'role':'test-only'if is_test else'production','chains':chains}
 source_paths.add('Cargo.toml');sources={p:{'length':len((R/p).read_bytes()),'sha256':sha((R/p).read_bytes())}for p in sorted(source_paths)}
 return {'schema':'semaprax.source-authority-partition.v1','parser_sha256':sha(parser.read_bytes()),'sources':sources,'classes':classes,'chunks':chunks,'closure_only':closure}
for name,chunks in [('hir',hir_chunks),('projection',projection_chunks)]:
 result=manifest(chunks);(O/(name+'.json')).write_text(json.dumps(result,separators=(',',':'))+'\n');print(name,'chunks',len(chunks),'production',sum(x['role']=='production'for x in result['classes'].values()),'testonly',sum(x['role']=='test-only'for x in result['classes'].values()),'bytes',(O/(name+'.json')).stat().st_size)
