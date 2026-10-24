#!/usr/bin/env python3
"""Bounded synthetic controls for current source-role registration transport."""
import importlib.util
from pathlib import Path

HERE=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('current_generation',HERE/'current_generation.py')
current=importlib.util.module_from_spec(spec);spec.loader.exec_module(current)

def edge(parent,child,text):return {'parent':parent,'start':0,'text':text,'attributes':[],'child':child}
def closed(kind,chain):return kind.startswith('test:') or any('#[cfg(test)]' in node['attributes'] for node in chain)
def continuous(root,trail,target):
 for item in trail:
  assert item['parent']==root;root=item['child']
 assert root==target

# A test-data alias cannot replace an independently registered production path.
records={'same.rs':[('lib',[], 'pkg',None)]}
alias=('test:harness',[], 'pkg',None)
assert current.inherit_record(records,'same.rs',alias,edge('tests/harness.rs','same.rs','include_str!("same.rs")'))
assert current.registration_role(records['same.rs'],closed)=='production'
assert {row[0] for row in records['same.rs']}=={'lib','test:harness'}

# Ordered trails preserve multihop compiled includes after a data carrier edge.
records={'carrier.rs':[alias]}
first=edge('tests/harness.rs','carrier.rs','include_str!("carrier.rs")')
current.inherit_record(records,'carrier.rs',alias,first)
seed=next(row for row in records['carrier.rs'] if current.record_trail(row))
second=edge('carrier.rs','middle.rs','include!("middle.rs")')
third=edge('middle.rs','leaf.rs','include!("leaf.rs")')
assert current.inherit_record(records,'middle.rs',seed,second)
assert current.inherit_record(records,'leaf.rs',records['middle.rs'][0],third)
trail=current.record_trail(records['leaf.rs'][0]);continuous('tests/harness.rs',trail,'leaf.rs');assert len(trail)==3
assert not current.inherit_record(records,'leaf.rs',records['middle.rs'][0],third)
# The actual data-alias constructor retains a pre-existing trail.
aliased=current.data_alias_records(records['middle.rs'],edge('middle.rs','data.rs','include_str!("data.rs")'))
assert len(current.record_trail(aliased[0]))==3;continuous('tests/harness.rs',current.record_trail(aliased[0]),'data.rs')
try:current.require_acyclic_extension(records['leaf.rs'][0],'leaf.rs','carrier.rs')
except AssertionError:pass
else:raise AssertionError('compiled include cycle accepted')
try:current.require_acyclic_extension(('lib',[],'pkg',None),'self.rs','self.rs')
except AssertionError:pass
else:raise AssertionError('self include accepted')

# cfg(test) registrations stay closed, while one real lib registration makes
# the same compiled path production without a role override.
cfg=[{'attributes':['#[cfg(test)]']}]
only_test=[('lib',cfg,'pkg',None),('test:harness',[],'pkg',None)]
assert current.registration_role(only_test,closed)=='test-only'
assert current.registration_role(only_test+[('lib',[],'pkg',None)],closed)=='production'
text='#[cfg(test)]\ninclude!("leaf.rs")';masked=text
# literal_include_edge consumes the ordinary re.Match API.
import re
m=re.search(r'include!\("([^"]+)"\)',text)
cfg_edge=current.literal_include_edge(text,masked,'parent.rs','leaf.rs',m.start(),m)
cfg_record=('lib',[],'pkg',(cfg_edge,))
assert cfg_edge['attributes']==['#[cfg(test)]'] and current.registration_role([cfg_record],closed)=='test-only'
# Correct edge text and continuity do not authorize a different child path.
try:current.literal_include_edge(text,masked,'parent.rs','wrong.rs',m.start(),m)
except AssertionError:pass
else:raise AssertionError('wrong literal include destination accepted')
# A compiled include prefix remains explicit when a closed reader acquires
# DATA already independently registered as production. No folded shortcut may
# turn the module's destination into the carrier or the final DATA target.
module_edge=edge('src/lib.rs','src/inputs.rs','mod inputs;')
carrier_edge=edge('src/inputs.rs','src/tail.rs','include!("tail.rs")')
data_edge=edge('src/tail.rs','src/data.rs','include_str!("data.rs")')
carrier_record=('lib',[], 'pkg',(module_edge,carrier_edge))
acquired=current.data_alias_records([carrier_record],data_edge)[0]
continuous('src/lib.rs',current.record_trail(acquired),'src/data.rs')
assert current.record_trail(acquired)==(module_edge,carrier_edge,data_edge)
assert current.record_trail(acquired)[0]['child']=='src/inputs.rs'
records={'src/data.rs':[('lib',[], 'pkg',None)]}
assert current.inherit_record(records,'src/data.rs',carrier_record,data_edge)
assert len(records['src/data.rs'])==2
assert current.registration_role(records['src/data.rs'],closed)=='production'
text='include_str!("data.rs")';match=re.search(r'include_str!\("([^"]+)"\)',text)
assert current.literal_include_edge(text,text,'src/tail.rs','src/data.rs',0,match)==data_edge
try:current.literal_include_edge(text,text,'src/tail.rs','src/inputs.rs',0,match)
except AssertionError:pass
else:raise AssertionError('folded DATA destination accepted')
# A compiled fragment may read its parent as DATA. Keep the complete read,
# but never execute that parent's include again through the DATA occurrence.
read_back=edge('src/tail.rs','src/inputs.rs','include_str!("inputs.rs")')
read_record=current.data_alias_records([carrier_record],read_back)[0]
assert current.record_trail(read_record)==(module_edge,carrier_edge,read_back)
assert not current.can_expand_literal_include(read_record,'src/inputs.rs','src/tail.rs')
assert current.record_trail(read_record)[-1]==read_back
assert current.can_expand_literal_include(read_record,'src/inputs.rs','src/new.rs')
try:current.can_expand_literal_include(carrier_record,'src/tail.rs','src/inputs.rs')
except AssertionError:pass
else:raise AssertionError('actual compiled cycle accepted at DATA boundary')
# The same module ancestry may identify a DATA witness without identifying a
# compiled descendant. DATA records remain classified but never add scan roots.
assert current.compiled_registration(carrier_record)
assert not current.compiled_registration(acquired)
assert not current.compiled_registration(read_record)
# Each manifest starts from the same immutable finite witness basis. Running
# one domain first must not feed its DATA aliases into another domain.
import ast,hashlib,tempfile
functions=ast.parse((HERE/'current_generation.py').read_text())
generate_node=next(node for node in functions.body if isinstance(node,ast.FunctionDef)and node.name=='generate')
finite_nodes=[node for node in generate_node.body if isinstance(node,ast.FunctionDef)and node.name in ('literal_acquisitions','manifest')]
# Exercise the actual bootstrap loop: already registered targets belong only
# to the later DATA pass, never to the reusable initial DATA seed authority.
seed_loop=next(node for node in generate_node.body if isinstance(node,ast.For)and ast.unparse(node.iter).startswith('literal_acquisitions('))
bootstrap_calls=[]
def bootstrap_acquisitions(targets,registered):
 bootstrap_calls.append(set(targets))
 return {target:[('test:harness',[],'pkg',(edge('src/reader.rs',target,'include_str!("target.rs")'),))]for target in targets}
bootstrap={'needed':{'src/already.rs','src/missing.rs'},'files':{'src/already.rs':[('test:harness',[],'pkg',None)]},'literal_acquisitions':bootstrap_acquisitions}
bootstrap['parsed_records']=tuple((p,tuple(rows))for p,rows in bootstrap['files'].items())
exec(compile(ast.Module(body=[seed_loop],type_ignores=[]),'bootstrap fixture','exec'),bootstrap)
assert bootstrap_calls==[{'src/missing.rs'}]
assert bootstrap['files']['src/already.rs']==[('test:harness',[],'pkg',None)]
assert len(current.record_trail(bootstrap['files']['src/missing.rs'][0]))==1
with tempfile.TemporaryDirectory()as directory:
 root=Path(directory).resolve();(root/'src').mkdir()
 source='include!("tail.rs");\nconst DATA: &str = include_str!("data.rs");\nconst LEAF: &str = include_str!("leaf.rs");\n'
 (root/'src/root.rs').write_text(source)
 (root/'src/tail.rs').write_text('const DATA: &str = include_str!("data.rs");\n')
 (root/'src/data.rs').write_text('const MORE: &str = include_str!("leaf.rs");\n')
 (root/'src/leaf.rs').write_text('const BACK: &str = include_str!("data.rs");\n')
 (root/'Cargo.toml').write_text('')
 origin=('test:harness',[],'pkg',None)
 data=edge('src/root.rs','src/data.rs','include_str!("data.rs")');data['start']=source.index(data['text'])
 leaf=edge('src/root.rs','src/leaf.rs','include_str!("leaf.rs")');leaf['start']=source.index(leaf['text'])
 seeds={'src/root.rs':(origin,),
        'src/data.rs':tuple(current.data_alias_records([origin],data)),
        'src/leaf.rs':tuple(current.data_alias_records([origin],leaf))}
 ns=dict(vars(current),R=root,seed_files=seeds,classification_witnesses={},terminal_witnesses={},closed=closed,
         source_text=lambda p:Path(p).read_text(),source_bytes=lambda p:Path(p).read_bytes(),
         mask=lambda text:text,parser_raw=b'sealed synthetic parser',
         sha=lambda raw:hashlib.sha256(raw).hexdigest(),
         roots={('pkg','test:harness'):'src/root.rs'},parsed_records=tuple(seeds.items()))
 # This fixture declares its one local source-text assembly edge. The real
 # three-pair declaration and multiplicity validator are exercised below.
 ns['DATA_PROJECTIONS']=frozenset([('src/data.rs','src/leaf.rs')])
 def fixture_projection(parent,target):
  literal='include_str!("leaf.rs")';item=edge(parent,target,literal)
  item['start']=(root/parent).read_text().index(literal);return [item]
 ns['projection_fragment_edges']=fixture_projection
 exec(compile(ast.Module(body=finite_nodes,type_ignores=[]),'finite fixture','exec'),ns)
 first=[{'path':'src/root.rs'},{'path':'src/data.rs'},{'path':'src/leaf.rs'}]
 second=[{'path':'src/root.rs'},{'path':'src/data.rs'}]
 a=ns['manifest'](first);b=ns['manifest'](second)
 assert b==ns['manifest'](second) and a==ns['manifest'](first)
 assert seeds==ns['seed_files'] and len(seeds['src/data.rs'])==1
 for model in (a,b):
  for cls in model['classes'].values():
   for chain in cls['chains']:
    assert sum(e['text'].startswith('include_str!')for e in chain['edges'])<=2
 assert any(sum(e['text'].startswith('include_str!')for e in c['edges'])==2 for c in a['classes']['src/leaf.rs']['chains'])
 # An already parsed target has no reusable DATA seed, yet its actual
 # direct read is still acquired by the one immutable-basis late pass.
 already=dict(seeds);already['src/data.rs']=(('test:already',[],'pkg',None),)
 ns['seed_files']=already;ns['roots'][('pkg','test:already')]='src/data.rs'
 model=ns['manifest'](first)
 assert already['src/data.rs']==(('test:already',[],'pkg',None),)
 assert any(any(e['parent']=='src/root.rs'and e['text']=='include_str!("data.rs")'for e in c['edges'])for c in model['classes']['src/data.rs']['chains'])
 # A compiled fragment can expose a separately registered structural ancestor.
 # Classify it before fixing DATA targets so its real direct read is retained.
 (root/'src/root.rs').write_text(source+'const ANCESTOR: &str = include_str!("ancestor.rs");\n')
 (root/'src/ancestor.rs').write_text('mod tail;\n')
 extended=dict(seeds)
 extended['src/ancestor.rs']=(('test:ancestor',[],'pkg',None),)
 ancestor_node={'source_path':'src/ancestor.rs','module':'tail','line':1,'attributes':[]}
 extended['src/tail.rs']=(('test:ancestor',[ancestor_node],'pkg',None),)
 ns['seed_files']=extended;ns['roots'][('pkg','test:ancestor')]='src/ancestor.rs'
 model=ns['manifest'](first)
 assert any(c['root']=='src/root.rs'and c['edges'][-1]['text']=='include_str!("ancestor.rs")'for c in model['classes']['src/ancestor.rs']['chains'])
 # A domain-admitted reader discovers a registered target absent from its raw
 # chunks. DATA does not turn that target's own include tokens into scan roots.
 (root/'src/registered.rs').write_text('include!(concat!("not", "a scan root"));\n')
 (root/'src/root.rs').write_text(source+'const REGISTERED: &str = include_str!("registered.rs");\n')
 registered=dict(seeds);registered['src/registered.rs']=(('test:registered',[],'pkg',None),)
 ns['seed_files']=registered;ns['parsed_records']=tuple(registered.items())
 ns['roots'][('pkg','test:registered')]='src/registered.rs'
 model=ns['manifest'](first)
 assert 'src/registered.rs'in model['classes']
 assert any(c['root']=='src/root.rs'and c['edges'][-1]['text']=='include_str!("registered.rs")'for c in model['classes']['src/registered.rs']['chains'])
 # Frozen early closed-reader witnesses remain terminal even if another
 # registration later prevents that reader joining the ordinary closed basis.
 reader_text='const LEAF: &str = include_str!("leaf.rs");\ninclude!(concat!("no", "scan"));\n'
 (root/'src/reader.rs').write_text(reader_text)
 reader=('test:reader',[],'pkg',None)
 frozen=edge('src/reader.rs','src/leaf.rs','include_str!("leaf.rs")')
 frozen['start']=reader_text.index(frozen['text'])
 ns['terminal_witnesses']={'src/leaf.rs':current.data_alias_records([reader],frozen)}
 mixed=dict(seeds);mixed['src/reader.rs']=(reader,('lib',[],'pkg',None))
 ns['seed_files']=mixed;ns['parsed_records']=tuple(mixed.items())
 ns['roots'][('pkg','test:reader')]='src/reader.rs';ns['roots'][('pkg','lib')]='src/reader.rs'
 model=ns['manifest'](first)
 assert any(c['root']=='src/reader.rs'and c['kind']=='test'and c['edges'][-1]['text']=='include_str!("leaf.rs")'for c in model['classes']['src/leaf.rs']['chains'])
# Classification-only aliases may admit an already parsed source class;
# they do not become reusable DATA seeds or compiled body scan roots.
manifest_node=next(n for n in finite_nodes if n.name=='manifest')
selection_loop=next(n for n in manifest_node.body if isinstance(n,ast.For)and ast.unparse(n.iter)=='files.items()')
node={'source_path':'src/raw.rs','module':'reader','line':1,'attributes':[]}
witness=('test:harness',[node],'pkg',(edge('src/reader.rs','src/target.rs','include_str!("target.rs")'),))
selection=dict(vars(current),files={'src/target.rs':[('test:harness',[],'pkg',None)]},classification_witnesses={'src/target.rs':[witness]},raw_paths={'src/raw.rs'},paths=set(),compiled_scan=set())
exec(compile(ast.Module(body=[selection_loop],type_ignores=[]),'classification fixture','exec'),selection)
assert 'src/target.rs'in selection['paths'] and 'src/target.rs'not in selection['compiled_scan']
assert selection['files']['src/target.rs']==[('test:harness',[],'pkg',None)]

# The continuation grammar is the exact declared three-pair source assembly
# contract, not arbitrary include tokens in a file read as DATA.
canonical=ast.parse((HERE.parent/'canonical-byte-source-20261022/current-source-authority.py').read_text())
declaration=next(n for n in canonical.body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='DATA_PROJECTIONS'for t in n.targets))
assert current.DATA_PROJECTIONS==frozenset(ast.literal_eval(declaration.value.args[0]))
projection_node=next(n for n in generate_node.body if isinstance(n,ast.FunctionDef)and n.name=='projection_fragment_edges')
with tempfile.TemporaryDirectory()as directory:
 root=Path(directory).resolve();texts={}
 for parent,target in current.DATA_PROJECTIONS:
  (root/parent).parent.mkdir(parents=True,exist_ok=True)
  if parent=='src/source_reflows_merge/main67107_inputs_tail.rs':
   literal='include_str!("../../tests/workspace/semantic_graph/source_inventory.rs")';text=literal+';\n'+literal+';\n'
  elif parent=='tests/workspace/semantic_graph.rs':text='include!("semantic_graph/source_inventory.rs");\n'
  else:text='include!("source_inventory_leaf_views.rs");\n'
  (root/parent).write_text(text);texts[parent]=text
 ns=dict(vars(current),R=root,source_text=lambda p:Path(p).read_text(),mask=lambda text:text,projection_cache={})
 exec(compile(ast.Module(body=[projection_node],type_ignores=[]),'projection fixture','exec'),ns)
 for parent,target in current.DATA_PROJECTIONS:
  expected=2 if parent=='src/source_reflows_merge/main67107_inputs_tail.rs'else 1
  assert len(ns['projection_fragment_edges'](parent,target))==expected
  for changed in ['',texts[parent]+texts[parent]]:
   ns['projection_cache'].clear();(root/parent).write_text(changed)
   try:ns['projection_fragment_edges'](parent,target)
   except AssertionError:pass
   else:raise AssertionError('missing/duplicate declared source-text occurrence accepted')
  (root/parent).write_text(texts[parent])
 try:ns['projection_fragment_edges']('src/arbitrary.rs','src/other.rs')
 except AssertionError:pass
 else:raise AssertionError('undeclared nested DATA read accepted')
print('current generation synthetic controls: PASS')
