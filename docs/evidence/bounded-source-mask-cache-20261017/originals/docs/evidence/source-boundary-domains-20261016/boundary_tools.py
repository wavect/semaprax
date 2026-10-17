"""Finite domain membership atop unchanged complete source-role authentication."""
from pathlib import Path
import copy,hashlib,json,re,subprocess,sys,tempfile
BASE='a82e00cc83f99b16249181c45710d962598577db'
DREL='docs/evidence/source-boundary-domains-20261016'
FREL='docs/evidence/a3-e47-final-custody-20261016'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(repo,ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=repo)
def protected(repo):
 d=repo/DREL;j=json.loads((d/'original-tool-bindings.json').read_text())
 for row in j['tools']:
  actual=(repo/row['source']).read_bytes();assert actual==(repo/row['archive']).read_bytes() and sha(actual)==row['sha256'],row['source']
 helper=(repo/j['helper_original']).read_bytes();assert helper==(repo/j['helper_archive']).read_bytes() and sha(helper)==j['helper_before_sha256']
 assert helper==git(repo,j['before_head'],'tests/support/production_source_inventory.rs')
 sealed=json.loads((d/'sealed-domains.json').read_text());result={}
 for row in sealed['domains']:
  assert row['baseline']==BASE and row['name'] in ['hir','projection'];assert row['source']==FREL+'/'+row['name']+'.json';assert row['original_complete_classes']=={'hir':1046,'projection':179}[row['name']];raw=(d/row['archive']).read_bytes();assert raw==git(repo,BASE,row['source']) and len(raw)==row['length'] and sha(raw)==row['sha256']
  original=json.loads(raw);roots=set(original['classes']);assert roots==set(row['scope_roots']) and len(roots)==row['original_complete_classes'];result[row['name']]=(row,original)
 assert set(result)=={'hir','projection'};return result

def original_verifier(repo):
 original=repo/FREL/'verify-authority.py';text=original.read_text();prefix=text.split('proof={}\n',1)[0];assert len(prefix)<len(text)
 ns={'__file__':str(original)};exec(compile(prefix,str(original),'exec'),ns);ns['R']=repo;ns['D']=repo/FREL
 return ns['verify'],sha(prefix.encode())

def original_generated(repo):
 # Exact original definitions and registration/inventory enumeration, stopping
 # only before its final output loop. This detects missing actual registered
 # descendants independently of the supplied current JSON classes.
 original=repo/FREL/'generate-authority.py';text=original.read_text();anchor="for name,chunks in [('hir',hir_chunks),('projection',projection_chunks)]:";prefix=text.split(anchor,1)[0];assert len(prefix)<len(text)
 with tempfile.TemporaryDirectory(prefix='boundary-register-',dir='/Volumes/Semaprax/runtime-tmp') as tmp:
  prior=sys.argv[:]
  try:
   sys.argv=[str(original),'--repo',str(repo),'--out',tmp];ns={'__file__':str(original)};exec(compile(prefix,str(original),'exec'),ns)
   results={name:ns['manifest'](ns[name+'_chunks']) for name in ['hir','projection']}
  finally:sys.argv=prior
 return results,sha(prefix.encode())

def compiled(text):
 text=text.strip()
 if text.startswith('include_str!'):return False
 if text.startswith('include!'):return True
 assert re.search(r'\bmod\s+(?:r#)?\w+\s*[;{]\s*$',text),('unknown actual registration',text)
 return True

def membership(full,roots):
 classes=full['classes'];assert roots<=set(classes),'lost original audited source';edges={}
 for row in classes.values():
  for chain in row['chains']:
   for edge in chain['edges']:
    if compiled(edge['text']):edges.setdefault(edge['parent'],set()).add(edge['child'])
 members=set(roots);todo=list(roots)
 while todo:
  for child in edges.get(todo.pop(),set()):
   assert child in classes,('unclassified actual boundary child',child)
   if child not in members:members.add(child);todo.append(child)
 return members

def scope(full,seal):
 row,baseline=seal;roots=set(baseline['classes']);members=membership(full,roots)
 production={p for p in members if full['classes'][p]['role']=='production'}
 outside={p for p,v in full['classes'].items() if v['role']=='production' and p not in members}
 return dict(baseline=BASE,baseline_sha256=row['sha256'],original_roots=sorted(roots),current_members=sorted(members),production_members=sorted(production),fully_authenticated_outside_production=sorted(outside))

def validate_scope(full,sealed,s):
 expected=scope(full,sealed)
 for key,value in expected.items():assert s[key]==value,('domain membership/role mismatch',key)
 return set(expected['current_members'])

def boundary_text(repo,full,members):
 text={p:(repo/p).read_text() for p in full['sources']};pieces=[]
 for chunk in full['chunks']:
  if 'literal'in chunk:pieces.append(chunk['literal'])
  elif chunk['path']in members and full['classes'][chunk['path']]['role']=='production':pieces.append(text[chunk['path']])
 for path in full['closure_only']:
  if path in members and full['classes'][path]['role']=='production':pieces.append(text[path])
 return ''.join(pieces)

def forbidden(repo,name):
 owner=(repo/('tests/hir_module_boundaries.rs'if name=='hir'else'tests/workspace/semantic_graph.rs')).read_text()
 anchor=owner.index('let validation = production_source_inventory'if name=='hir'else'let authority_projection = production_projection')
 block=owner[owner.index('for forbidden in [',anchor):];block=block[:block.index('] {')];tokens=re.findall(r'"([^"\n]*)"',block)
 return (['use super::*;']if name=='projection'else[])+tokens

def generated_scope(repo,full_dir,regenerated=None):
 sealed=protected(repo);verify,verify_sha=original_verifier(repo);generated,generator_sha=regenerated if regenerated is not None else original_generated(repo);domains={};proof={}
 for name in ['hir','projection']:
  file=full_dir/(name+'.json');raw=file.read_bytes();full=json.loads(raw)
  assert full==generated[name],('full current manifest lost actual original-parser registration/include closure',name)
  whole,all_production,production,test=verify(full)
  s=scope(full,sealed[name]);s['full_manifest_sha256']=sha(raw);s['full_raw_sha256']=sha(whole.encode());s['full_authenticated_production_sha256']=sha(all_production.encode());domains[name]=s
  actual=boundary_text(repo,full,set(s['current_members']));bad=[x for x in forbidden(repo,name) if x in actual];assert not bad,(name,'actual owning boundary forbidden',bad)
  proof[name]=dict(original_roots=len(s['original_roots']),current_boundary_members=len(s['current_members']),current_boundary_production=len(s['production_members']),outside_production_still_fully_authenticated=len(s['fully_authenticated_outside_production']),full_role_production=len(production),full_role_test_only=len(test),full_raw_sha256=s['full_raw_sha256'],boundary_production_sha256=sha(actual.encode()),exact_original_forbidden_tokens=forbidden(repo,name),all_original_coverage_preserved=True,actual_current_registered_children_complete=True,global_polluted_purity_result_not_claimed=True)
 return dict(schema='semaprax.source-boundary-domains.v1',status='SOURCE_ONLY_GENERATED_NOT_RUST_QUALIFICATION',domains=domains),dict(mode='SOURCE_ONLY_NO_RUST_EXECUTION',original_generator_definition_span_sha256=generator_sha,original_verifier_definition_span_sha256=verify_sha,domains=proof)

def negative_checks(repo,full_dir,domains):
 sealed=protected(repo);verify,_=original_verifier(repo)
 for name in ['hir','projection']:
  full=json.loads((full_dir/(name+'.json')).read_text());s=domains['domains'][name];members=set(s['current_members']);roots=set(s['original_roots']);child=next(iter(sorted(members-roots)))
  broken=copy.deepcopy(s);broken['current_members'].remove(child)
  try:validate_scope(full,sealed[name],broken)
  except AssertionError:pass
  else:raise AssertionError('dropped actual current child accepted')
  # Even coordinated removal from classes cannot pass the independently
  # regenerated complete original-parser output or the sealed full-manifest hash.
  changed=copy.deepcopy(full);del changed['classes'][child];changed['closure_only']=[p for p in changed['closure_only']if p!=child]
  assert changed!=full and sha((json.dumps(changed,separators=(',',':'))+'\n').encode())!=s['full_manifest_sha256']
  production=next(p for p,v in full['classes'].items()if v['role']=='production');changed=copy.deepcopy(full);changed['classes'][production]['role']='test-only'
  try:verify(changed)
  except AssertionError:pass
  else:raise AssertionError('production reclassification accepted')
  before=(repo/production).read_bytes()
  try:verify(full,{production:before+b' '})
  except AssertionError:pass
  else:raise AssertionError('actual source drift accepted')
  raw=verify(full)[0];assert sha((raw+'\n').encode())!=s['full_raw_sha256']
  # A declared #[path] child outside its parent's directory remains included.
  parent=next(p for p in roots if full['classes'][p]['role']=='production');changed=copy.deepcopy(full);chain=copy.deepcopy(changed['classes'][parent]['chains'][0]);foreign='src/foreign_boundary_child.rs';chain['edges'].append(dict(parent=parent,child=foreign,text='#[path = "../foreign_boundary_child.rs"] mod relocated;',attributes=['#[path = "../foreign_boundary_child.rs"]'],start=0));changed['classes'][foreign]=dict(role='production',chains=[chain]);assert foreign in membership(changed,roots)
  try:validate_scope(changed,sealed[name],s)
  except AssertionError:pass
  else:raise AssertionError('foreign-path actual child escaped sealed membership')
 return dict(dropped_actual_child_refused=True,coordinated_class_drop_not_original_generated_output=True,production_reclassification_refused=True,actual_source_drift_refused=True,whole_raw_drift_refused=True,foreign_path_child_cannot_escape=True)
