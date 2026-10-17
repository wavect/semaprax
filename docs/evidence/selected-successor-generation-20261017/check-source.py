"""Pure whole source and finite binder controls; no generator/main execution."""
from pathlib import Path
import ast,hashlib,json,subprocess,importlib.util
D=Path(__file__).resolve().parent;R=D.parents[2]
sha=lambda raw:hashlib.sha256(raw).hexdigest()
rows=json.loads((D/'full-source-inverses.json').read_bytes())
refused=0
for row in rows:
    old=(R/row['archive']).read_bytes();raw=(R/row['path']).read_bytes()
    assert len(old)==row['before_length'] and sha(old)==row['before_sha256']
    assert old==subprocess.check_output(['git','show','f6aeccc21d16e50a78d4f44522eaa5b028bd2cc0:'+row['path']],cwd=R)
    def restore(raw):
        assert len(raw)==row['current_length'] and sha(raw)==row['current_sha256']
        text=raw.decode()
        for edit in row['inverse']:
            assert text.count(edit['after'])==1;text=text.replace(edit['after'],edit['before'],1)
        assert text.encode()==old
        return text
    restore(raw)
    for bad in [raw+b' ',raw[:-1],b'foreign',old,raw.replace(b'SelectedReservation',b'LegacyUnobserved',1)]:
        try:restore(bad)
        except AssertionError:refused+=1
        else:raise AssertionError('foreign stage accepted')
    assert len(raw.splitlines())<=1500
root=(R/rows[0]['path']).read_text();prior=(R/rows[0]['archive']).read_text()
assert root.replace('                AllocationPayer::SelectedReservation => {}\n','').replace('            AllocationPayer::SelectedReservation => {}\n','')==prior
assert root.count('return borrowed_labels::ordinary_successors(')==prior.count('return borrowed_labels::ordinary_successors(')==2
assert root.count('return borrowed_labels::successors(')==prior.count('return borrowed_labels::successors(')==2
new=(R/rows[1]['path']).read_text();old=(R/rows[1]['archive']).read_text();assert new.startswith(old)
for start,end in [('    fn successors(\n','    #[cfg(test)]\n    fn closure('),('    fn successors_with_receiving(\n','    pub(super) fn id_with_receiving(')]:
    offset=prior.index(start);body=prior[offset:offset+prior[offset:].index(end)]
    renamed=body.replace('fn successors(','fn previous_selected_successors(').replace('fn successors_with_receiving(','fn previous_selected_successors_with_receiving(').replace('self.successors_with_receiving(','self.previous_selected_successors_with_receiving(')
    assert new.count(renamed)==1
spec=importlib.util.spec_from_file_location('selected_bind_controls',D/'bind-selected-endpoints.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
ast.parse(m.render());assert m.selected_previous(R,root.encode())==prior.encode()
for raw in [prior.encode(),root[:-1].encode(),root.encode()+b' ',root.replace('SelectedReservation','LegacyUnobserved',1).encode()]:
    try:m.selected_previous(R,raw)
    except AssertionError:refused+=1
    else:raise AssertionError('binder bypass')
for row in json.loads((D/'pending-selectors.json').read_bytes())['checks']:
    assert new.count('fn '+row['selector'].split('::')[-1]+'(')==1
print(json.dumps({'status':'PURE_SOURCE_PASS_NOT_RUST_QUALIFIED','whole_preimages':2,'complete_compiled_predecessor_methods':2,'new_rust_physical_owners':0,'hostile_stage_refusals':refused,'binder_main_executed':False}))
