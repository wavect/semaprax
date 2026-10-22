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
print('current generation synthetic controls: PASS')
