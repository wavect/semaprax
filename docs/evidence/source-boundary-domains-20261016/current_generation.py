"""Current source-role generation with exact legacy parser semantics and source snapshots."""
from pathlib import Path
import argparse,json,re,hashlib,tomllib,posixpath

# Owning source-as-text grammar: canonical-byte-source-20261022/current-source-authority.py:104-151.
# These declarations admit complete read-only fragment trails, never folded routes.
DATA_PROJECTIONS=frozenset([
 ('src/source_reflows_merge/main67107_inputs_tail.rs','tests/workspace/semantic_graph/source_inventory.rs'),
 ('crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs'),
 ('tests/workspace/semantic_graph.rs','tests/workspace/semantic_graph/source_inventory.rs'),
])

def record_trail(record):
 value=record[3]
 return () if value is None else value if isinstance(value,tuple) else (value,)

def inherit_record(records,target,record,edge):
 candidate=(record[0],record[1],record[2],record_trail(record)+(edge,))
 rows=records.setdefault(target,[])
 if candidate not in rows:rows.append(candidate);return True
 return False

def data_alias_records(registrations,edge):
 rows=[]
 for record in registrations:inherit_record({'target':rows},'target',record,edge)
 return rows

def registration_paths(record):
 paths={node['source_path'] for node in record[1]}
 for edge in record_trail(record):paths.update((edge['parent'],edge['child']))
 return paths

def require_acyclic_extension(record,parent,target):
 assert target!=parent and target not in registration_paths(record),('compiled include cycle',parent,target)

def compiled_registration(record):
 return not any(edge['text'].lstrip().startswith('include_str!') for edge in record_trail(record))

def can_expand_literal_include(record,parent,target):
 # An include_str! is a terminal source read, not another execution of the
 # source's include tokens. Retain that exact read; stop a repeated DATA path.
 if any(edge['text'].lstrip().startswith('include_str!') for edge in record_trail(record)) and target in registration_paths(record):return False
 require_acyclic_extension(record,parent,target)
 return True

def registration_role(records,closed):
 def is_test(record):
  return closed(record[0],record[1]) or any(any(re.fullmatch(r'#\[\s*cfg\s*\(\s*test\s*\)\s*\]',a)for a in edge['attributes'])for edge in record_trail(record))
 return 'test-only' if all(is_test(record)for record in records) else 'production'

def literal_include_edge(text,masked,parent,target,start,match):
 assert not Path(match[1]).is_absolute() and posixpath.normpath(str(Path(parent).parent/match[1]))==target,('wrong literal include destination',parent,target,match[1])
 attributes=[];attribute_start=start
 block=re.search(r'((?:#\[[^\]]+\]\s*)*)$',masked[:start]).group(1)
 if block:
  attribute_start=start-len(block);attributes=re.findall(r'#\[[^\]]+\]',text[attribute_start:start])
 return {'parent':parent,'start':len(text[:attribute_start].encode()),'text':text[attribute_start:start+len(match[0])],'attributes':attributes,'child':target}

def generate(repo,out=None):
 R=Path(repo).resolve();assert R.is_dir();D=R/'docs/evidence/production-source-authority-partition-20261016';O=Path(out).resolve() if out is not None else None

 # Performance-only memoization. Every path is opened without symlink following,
 # checked before/after its complete read, and revalidated before publication.
 import os,stat
 _cache_bytes={}
 _cache_text={}
 _cache_ident={}
 def _ident(st): return (st.st_dev,st.st_ino,st.st_mode,st.st_size,st.st_mtime_ns,st.st_ctime_ns)
 def source_bytes(path,cap=16777216):
  p=Path(path).resolve();assert p.is_relative_to(R);key=str(p)
  if key in _cache_bytes:return _cache_bytes[key]
  fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC)
  try:
   before=os.fstat(fd); assert stat.S_ISREG(before.st_mode) and before.st_size<=cap,(key,before.st_size)
   with os.fdopen(fd,'rb',closefd=False) as f:
    raw=f.read(before.st_size); assert len(raw)==before.st_size and not f.read(1),key
   after=os.fstat(fd); path_after=p.lstat(); assert _ident(before)==_ident(after)==_ident(path_after),('source drift',key)
  finally: os.close(fd)
  _cache_bytes[key]=raw;_cache_ident[key]=_ident(before);return raw
 def source_text(path):
  key=str(Path(path).resolve())
  if key not in _cache_text:_cache_text[key]=source_bytes(path).decode()
  return _cache_text[key]
 def verify_cached_sources():
  for key,ident in _cache_ident.items():
   p=Path(key);fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC)
   try:
    before=os.fstat(fd);assert _ident(before)==ident,('source drift before final read',key)
    with os.fdopen(fd,'rb',closefd=False) as f:
     raw=f.read(before.st_size);assert len(raw)==before.st_size and not f.read(1),key
    after=os.fstat(fd);final=p.lstat()
    assert _ident(before)==_ident(after)==_ident(final)==ident,('source drift during final read',key)
    assert raw==_cache_bytes[key],('source bytes changed before publication',key)
   finally:os.close(fd)
 sha=lambda b:hashlib.sha256(b).hexdigest();
 ns={};parser=D/'original-module-parser-prefix.py';parser_raw=source_bytes(parser);parser_text=parser_raw.decode();sha_read='def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()';text_read='s=p.read_text();m=mask(s)';assert parser_text.count(sha_read)==parser_text.count(text_read)==1;parser_exec=parser_text.replace(sha_read,'def sha(p):return hashlib.sha256(source_bytes(p)).hexdigest()').replace(text_read,'s=source_text(p);m=mask(s)');ns.update(source_bytes=source_bytes,source_text=source_text);exec(compile(parser_exec,str(parser),'exec'),ns);_raw_mask=ns['mask'];_mask_cache={}
 def _cached_mask(text):
  key=id(text);row=_mask_cache.get(key)
  if row is None:row=(text,_raw_mask(text));_mask_cache[key]=row
  else:assert row[0] is text
  return row[1]
 ns['mask']=_cached_mask;ns['R']=R;original=ns['parse'];files={}
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
  assert s.count(JOIN)==1, ('audit_source_join definition multiplicity', s.count(JOIN))
  return s.replace(JOIN, '', 1).replace('audit_source_join!', 'concat!')
 inv=inventory_transport(source_text(R/'tests/workspace/semantic_graph/source_inventory.rs'));im=mask(inv);macros={}
 for t in re.finditer(r'macro_rules!\s+(\w+)\s*\{',im):
  end=close(im,t.end()-1,'{','}');body=inv[t.end():end-1];bm=mask(body);arms={}
  for v in re.finditer(r'\(([^)]*)\)\s*=>\s*\{',bm):arms[v[1].strip()]=body[v.end():close(bm,v.end()-1,'{','}')-1]
  macros[t[1]]=arms
 hir_inv=inventory_transport(source_text(R/'tests/hir_module_boundaries.rs'));hir_mask=mask(hir_inv)
 for t in re.finditer(r'macro_rules!\s+(\w+)\s*\{',hir_mask):
  end=close(hir_mask,t.end()-1,'{','}');body=hir_inv[t.end():end-1];bm=mask(body);arms={}
  for v in re.finditer(r'\(([^)]*)\)\s*=>\s*\{',bm):arms[v[1].strip()]=body[v.end():close(bm,v.end()-1,'{','}')-1]
  assert t[1] not in macros or macros[t[1]]==arms,('ambiguous literal inventory macro',t[1])
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
 hir=inventory_transport(source_text(R/'tests/hir_module_boundaries.rs'));at=hir.index('concat!(',hir.index('let validation_raw'));hir_chunks=Expr(hir[at:],R/'tests').expr()
 projection_parts=[('semantic_graph_private_layout_owned_source_inventory',''),('semantic_graph_projection_child_source_inventory','type_rewrite'),('semantic_graph_projection_child_source_inventory','import_stub'),('semantic_graph_projection_child_source_inventory','uncached_peak'),('semantic_graph_projection_root_source_inventory','')]+[('semantic_graph_projection_child_source_inventory',x) for x in ['local_identity','call_identity','call_sites','cost','declaration_cost','identity_slots','statement_segment','defaults']]
 projection_chunks=[]
 for i,(n,k) in enumerate(projection_parts):
  if i:projection_chunks.append({'literal':'\n'})
  projection_chunks+=Expr(macros[n][k],R/'tests/workspace').expr()
 needed={x['path'] for x in hir_chunks+projection_chunks if 'path'in x}
 # Every closed literal acquisition is retained even when the target also
 # has an independent parsed registration. A complete prefix is never folded
 # into a different module destination.
 def literal_acquisitions(targets,registered):
  by_target={target:[] for target in sorted(targets)};names={Path(target).name for target in targets}
  for parent,registrations in registered:
   if not all(closed(record[0],record[1]) for record in registrations):continue
   text=source_text(R/parent)
   if not any(name in text for name in names):continue
   masked=mask(text)
   for m in re.finditer(r'\binclude(?:_str)?!\(\s*"([^"]+)"\s*\)',text):
    if not masked.startswith('include',m.start()):continue
    target=str(((R/parent).parent/m[1]).resolve().relative_to(R))
    if target not in by_target:continue
    edge={'parent':parent,'start':len(text[:m.start()].encode()),'text':m[0],'attributes':[],'child':target}
    for record in registrations:inherit_record(by_target,target,record,edge)
  return by_target
 # Classification witnesses are terminal reads, not reusable DATA seeds.
 parsed_records=tuple((p,tuple(rows))for p,rows in files.items())
 terminal_witnesses=literal_acquisitions(needed|set(files),parsed_records)
 classification_witnesses={p:terminal_witnesses[p]for p in needed}
 for target,candidates in literal_acquisitions(needed-set(files),parsed_records).items():
  assert candidates or target in files,('unclassified inventory source',target)
  for record in candidates:
   rows=files.setdefault(target,[])
   if record not in rows:rows.append(record)
 seed_files={p:tuple(rows)for p,rows in files.items()}
 projection_cache={}
 def projection_fragment_edges(parent,target):
  assert (parent,target)in DATA_PROJECTIONS,('undeclared source-text continuation',parent,target)
  pair=(parent,target)
  if pair in projection_cache:return projection_cache[pair]
  text=source_text(R/parent);masked=mask(text);edges=[]
  data=parent=='src/source_reflows_merge/main67107_inputs_tail.rs'
  pattern=r'\binclude_str!\s*\('if data else r'\binclude!\s*\('
  literal=r'include_str!\s*\(\s*"([^"]+)"\s*\)'if data else r'include!\s*\(\s*"([^"]+)"\s*\)'
  for use in re.finditer(pattern,masked):
   match=re.match(literal,text[use.start():])
   if match is None:continue
   if str(((R/parent).parent/match[1]).resolve().relative_to(R))!=target:continue
   edges.append(literal_include_edge(text,masked,parent,target,use.start(),match))
  assert len(edges)==(2 if data else 1),('source-text literal multiplicity',parent,target,len(edges))
  projection_cache[pair]=tuple(edges);return projection_cache[pair]
 roots={('semaprax','lib'):'src/lib.rs',('semaprax','bin:semaprax'):'src/main.rs',('semaprax','test:hir_module_boundaries'):'tests/hir_module_boundaries.rs',('semaprax','test:workspace'):'tests/workspace.rs',('semaprax','test:wasm'):'tests/wasm.rs',('semaprax','test:project'):'tests/project.rs',('semaprax','test:cli_help_surface_v1'):'tests/cli_help_surface_v1.rs',('semaprax-toolchain','test:cli_help_surface_v1'):'crates/semaprax-toolchain/tests/cli_help_surface_v1.rs'}
 def manifest(chunks):
  files={p:list(rows)for p,rows in seed_files.items()}
  raw_paths={x['path']for x in chunks if 'path'in x}
  paths=set(raw_paths)
  compiled_scan=set(raw_paths)
  for path,records in files.items():
   selection=tuple(records)+tuple(classification_witnesses.get(path,()))
   descendants=[record for record in selection if any(node['source_path']in raw_paths for node in record[1])]
   if descendants:paths.add(path)
   if any(compiled_registration(record)for record in descendants):compiled_scan.add(path)
  # Inherit every parent registration across each literal compiled include.
  # The edge tuple retains multihop continuity and keeps data aliases distinct
  # from an independently parsed production registration of the same path.
  # Classify every actual acquisition ancestor without interpreting arbitrary
  # ancestor macro bodies as additional compiled inventory roots.
  while True:
   parents={node['source_path'] for path in paths for record in files[path] for node in record[1]}
   missing=parents-paths
   if not missing:break
   assert missing<=set(files),('unregistered module ancestor',missing)
   paths.update(missing)
  seen_counts={};include_edges={}
  def expand_includes(pending,discover):
   while pending:
    parent=pending.pop();records=files.get(parent);assert records,('unregistered inventory path',parent)
    if seen_counts.get(parent)==len(records):continue
    seen_counts[parent]=len(records)
    if parent not in include_edges:
     assert discover,('DATA cannot introduce a new include scan root',parent)
     text=source_text(R/parent);masked=mask(text);include_edges[parent]=[]
     for use in re.finditer(r'\binclude!\s*\(',masked):
      match=re.match(r'include!\s*\(\s*"([^"]+)"\s*\)',text[use.start():]);assert match,('nonliteral compiled include requires explicit inventory',parent)
      target=str(((R/parent).parent/match[1]).resolve().relative_to(R))
      include_edges[parent].append(literal_include_edge(text,masked,parent,target,use.start(),match))
    for edge in include_edges[parent]:
     target=edge['child'];paths.add(target);changed=False
     for record in list(records):
      if not compiled_registration(record):continue
      if not can_expand_literal_include(record,parent,target):continue
      changed=inherit_record(files,target,record,edge)or changed
     if changed or seen_counts.get(target)!=len(files[target]):pending.append(target)
  expand_includes(sorted(compiled_scan),True)
  # Freeze the admitted literal suffix DAG before acquiring any later DATA.
  active=set();complete=set()
  def check_dag(parent):
   assert parent not in active,('compiled include cycle',parent)
   if parent in complete:return
   active.add(parent)
   for edge in include_edges.get(parent,[]):check_dag(edge['child'])
   active.remove(parent);complete.add(parent)
  for parent in include_edges:check_dag(parent)
  # Discover terminal reads of already registered modules from this domain's
  # admitted compiled readers only. A target adds a class, never a scan root.
  admitted=compiled_scan|set(include_edges)
  discovery_basis=tuple((p,tuple(record for record in files[p]if compiled_registration(record)))for p in sorted(admitted))
  for target,candidates in literal_acquisitions({p for p,_ in parsed_records},discovery_basis).items():
   if not candidates:continue
   paths.add(target)
   for record in candidates:
    rows=files[target]
    if record not in rows:rows.append(record)
  # Close structural classes exposed by the compiled DAG before fixing the
  # targets of the single DATA pass. This adds no include scan root.
  while True:
   parents={node['source_path']for path in paths for record in files[path]for node in record[1]}
   parents.update(endpoint for path in paths for record in files[path]for edge in record_trail(record)for endpoint in (edge['parent'],edge['child']))
   missing=parents-paths
   if not missing:break
   assert missing<=set(files),('unregistered compiled acquisition ancestor',missing)
   paths.update(missing)
  # A finite immutable basis: original parsed/initial DATA witnesses plus real
  # DATA-free compiled include registrations. Derived DATA is never re-enlisted.
  late_basis={p:list(rows)for p,rows in seed_files.items()}
  for parent,records in files.items():
   rows=late_basis.setdefault(parent,[])
   for record in records:
    if compiled_registration(record) and record not in rows:rows.append(record)
  registered=tuple((p,tuple(record for record in rows if compiled_registration(record)))for p,rows in late_basis.items())
  for target,candidates in literal_acquisitions(paths,registered).items():
   for record in candidates:
    rows=files.setdefault(target,[])
    if record not in rows:rows.append(record)
  # At most one suffix propagation over the already frozen include DAG.
  expand_includes(list(seen_counts),False)
  # DATA text is terminal except for the three owning source-inventory
  # assembly declarations. Preserve each complete acquisition+fragment trail;
  # never scan arbitrary nested read tokens or manufacture a folded shortcut.
  for parent,target in sorted(DATA_PROJECTIONS):
   if parent not in files or target not in paths:continue
   for edge in projection_fragment_edges(parent,target):
    for record in tuple(files[parent]):
     trail=record_trail(record)
     if not trail or not trail[-1]['text'].lstrip().startswith('include_str!'):continue
     assert trail[-1]['child']==parent and closed(record[0],record[1]),('source-text projection must remain closed',parent,target)
     inherit_record(files,target,record,edge)
  # Late acquisitions can expose additional real module ancestors. Classify
  # them completely without adding any scan root or interpreting their bodies.
  while True:
   parents={node['source_path']for path in paths for record in files[path]for node in record[1]}
   parents.update(endpoint for path in paths for record in files[path]for edge in record_trail(record)for endpoint in (edge['parent'],edge['child']))
   missing=parents-paths
   if not missing:break
   assert missing<=set(files),('unregistered late module ancestor',missing)
   paths.update(missing)
  # Preserve the frozen early closed-reader occurrences as terminal witnesses
  # for the complete selected class set. They never become traversal inputs.
  for target in sorted(paths):
   for record in terminal_witnesses.get(target,()):
    if record not in files[target]:files[target].append(record)
  while True:
   parents={node['source_path']for path in paths for record in files[path]for node in record[1]}
   parents.update(endpoint for path in paths for record in files[path]for edge in record_trail(record)for endpoint in (edge['parent'],edge['child']))
   missing=parents-paths
   if not missing:break
   assert missing<=set(files),('unregistered terminal acquisition ancestor',missing)
   paths.update(missing)
  closure=sorted(paths-raw_paths);classes={};source_paths=set(paths)
  for p in sorted(paths):
   chains=[]
   for record in files[p]:
    kind,chain=record[:2];trail=record_trail(record);edges=[]
    for i,node in enumerate(chain):
     parent=node['source_path'];text=source_text(R/parent);masked=mask(text);line_start=sum(len(x)for x in text.splitlines(True)[:node['line']-1]);m=re.search(r'(?:pub(?:\([^)]*\))?\s+)?mod\s+'+re.escape(node['module'])+r'\s*[;{]',masked[line_start:]);assert m;pos=line_start+m.start();end=line_start+m.end();start=pos
     for attribute in reversed(node['attributes']):start=text.rfind(attribute,0,start);assert start>=0
     child=chain[i+1]['source_path']if i+1<len(chain)else (trail[0]['parent']if trail else p)
     edges.append({'parent':parent,'start':len(text[:start].encode()),'text':text[start:end],'attributes':node['attributes'],'child':child})
    edges.extend(trail);root=roots[(record[2],kind)];current=root
    for edge in edges:
     assert current==edge['parent'],('discontinuous registration chain',p,current,edge)
     literal=edge['text'].encode();raw=source_bytes(R/current);assert raw[edge['start']:edge['start']+len(literal)]==literal,('registration edge drift',p,edge)
     current=edge['child'];source_paths.update([edge['parent'],edge['child']])
    assert current==p,('registration terminal drift',p,current)
    value={'root':root,'kind':'lib'if kind=='lib'else 'bin' if kind.startswith('bin:') else 'test','package':record[2],'edges':edges};source_paths.add(root)
    if value not in chains:chains.append(value)
   classes[p]={'role':registration_role(files[p],closed),'chains':chains}
  source_paths.add('Cargo.toml');sources={p:{'length':len(source_bytes(R/p)),'sha256':sha(source_bytes(R/p))}for p in sorted(source_paths)}
  return {'schema':'semaprax.source-authority-partition.v1','parser_sha256':sha(parser_raw),'sources':sources,'classes':classes,'chunks':chunks,'closure_only':closure}
 results={name:manifest(chunks)for name,chunks in [('hir',hir_chunks),('projection',projection_chunks)]}
 verify_cached_sources()
 if O is not None:
  O.mkdir(parents=True,exist_ok=True)
  for name,result in results.items():(O/(name+'.json')).write_text(json.dumps(result,separators=(',',':'))+'\n')
 return results

def main():
 p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--out',type=Path);a=p.parse_args();generate(a.repo,a.out)
if __name__=='__main__':main()
