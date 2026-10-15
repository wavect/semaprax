#!/usr/bin/env python3
"""Bounded named source-domain joins; never executes Rust or old mutable tools."""
import json,hashlib,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
h=lambda b:hashlib.sha256(b).hexdigest()
g=lambda *a:subprocess.check_output(['git',*a],cwd=R)
def load(path):return json.loads((R/'docs/evidence'/path).read_text())
def apply(rows,path,text):
 hits=[r for r in rows if r['source']==path];assert len(hits)<=1
 if not hits:return text
 row=hits[0];assert h(text.encode())==row['current_sha256'],path
 for e in row['edits']:
  assert text.count(e['new'])==1,path;text=text.replace(e['new'],e.get('old',e.get('before')),1)
 assert h(text.encode())==row['original_sha256'],path
 if 'archive' in row:assert text.encode()==(R/row['archive']).read_bytes(),path
 return text
views={}
for stage in ['d8','844','846','836','main']:
 rows=json.loads((D/f'final-to-{stage}.json').read_text());rows=rows['assembly'] if stage=='836' else rows;views[stage]=rows
 for row in rows:
  b=apply(rows,row['source'],(R/row['source']).read_text())
  assert b.encode()==g('show',row['checkpoint_revision']+':'+row['source'])
def stage(stage,path):return apply(views[stage],path,(R/path).read_text())
# Original #844 table: final->immediate844->a3, followed by unchanged all26 inverses.
curtoa3=load('compact-automaton-844-immediate7a87/current-to-a3.json')
for row in load('compact-automaton-844/source-inverses.json'):
 path=row['source'];text=apply(curtoa3,path,stage('844',path));apply([row],path,text)
for row in load('compact-automaton-844/old-receiver-adaptations.json'):
 text=apply(curtoa3,row['source'],stage('844',row['source']));assert text==row['compiled'];assert h(text.encode())==row['compiled_sha256']
for row in load('compact-automaton-844/new-owners.json'):
 text=apply(curtoa3,row['source'],stage('844',row['source']));assert h(text.encode())==row['sha256']
for row in load('compact-automaton-844-immediate7a87/immediate-old-receivers.json'):
 text=stage('844',row['source']);assert text==row['compiled'];assert h(text.encode())==row['compiled_sha256'];assert (R/row['archive']).read_text()==row['original']
for row in load('compact-automaton-844-immediate7a87/new-owners.json'):assert h(stage('844',row['source']).encode())==row['sha256']
# #846 all original ten immediate inputs and three new owners.
for row in load('diagnostic-writer-846/current-to-7a87.json'):apply([row],row['source'],stage('846',row['source']))
for row in load('diagnostic-writer-846/new-owners.json'):assert h(stage('846',row['source']).encode())==row['sha256']
# #836 actual integration guard: independent final->a8->backend/coverage full tables.
bviews=load('record-field-while-836-20261015/backend-input-views.json');checks=0
for domain,packet in [('backends','record-copy-field-update-836-backends'),('coverage','record-copy-field-update-836-backend-controls')]:
 data=load(packet+'/source-inverses.json')
 for row in data['owners']:
  text=apply(bviews[domain],row['source'],stage('836',row['source']));apply([row],row['source'],text);checks+=1
 for row in data['children']:
  text=apply(bviews[domain],row['source'],stage('836',row['source']));assert h(text.encode())==row['sha256'];checks+=1
# Every actual MAIN input remains raw-byte-exact, and its prior11 source joins remain reproducible.
for row in load('main1e3-source-integration-20261015/exact-main-inputs.json'):assert h((R/row['source']).read_bytes())==row['sha256']
for row in load('main1e3-source-integration-20261015/current-to-7a87.json'):apply([row],row['source'],stage('main',row['source']))
# The concurrent pin/guard checkpoint is separate from MAIN and every feature checkpoint.
for row in load('actual-pin-consumer-7a87-20261015/current-to-92.json'):apply([row],row['source'],stage('d8',row['source']))
# All original protected runtime/evidence packet trees remain byte-exact to their source commits.
protected=[('b715fa258e27354cc1a85f652caa6a909f57239f','compact-automaton-844'),('b715fa258e27354cc1a85f652caa6a909f57239f','compact-automaton-844-immediate7a87'),('e2521dac782fa887da005bdba632dd31efb3f8aa','diagnostic-writer-846'),('a8d3d1a930d9c55eed20d554b4fcd9edaee79456','record-field-while-836-20261015'),('e0bb6de0cee445ccbd10bc349aa67d75748f997c','main1e3-source-integration-20261015'),('d8fadfb7cd5ece54504cda542575e5a154d3657c','actual-pin-consumer-7a87-20261015')]
kept=0
for rev,path in protected:
 base='docs/evidence/'+path
 for name in g('ls-tree','-r','--name-only',rev,'--',base).decode().splitlines():assert (R/name).read_bytes()==g('show',rev+':'+name),name;kept+=1
# New source owners from all three lanes/main join all three inventories exactly once.
children=[r['source'] for r in load('compact-automaton-844-immediate7a87/new-owners.json')]+[r['source'] for r in load('diagnostic-writer-846/new-owners.json')]+['src/cleanup_plan/replay/expression_facts/'+p for p in ['arena/before_cursor.rs','arena/producer_before_cursor.rs','arena/cursor_tests.rs','capacity/cursor_tests.rs']]
for path in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 text=(R/path).read_text()
 for child in children:assert text.count(child[4:]+'"')==1,(path,child)
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
print(json.dumps({'status':'SOURCE_ONLY_JOINED_NOT_QUALIFIED','named_view_rows':{k:len(v) for k,v in views.items()},'full_844_original_rows':26,'immediate_844_old_receivers':5,'new_846_owner_bindings':3,'old_836_full_input_checks':checks,'actual_MAIN_inputs_byte_exact':20,'old_packet_files_byte_exact':kept,'new_children_in_each_of_three_audits':len(children),'rust_execution':False,'new_head_pin_proof':False},indent=2))
