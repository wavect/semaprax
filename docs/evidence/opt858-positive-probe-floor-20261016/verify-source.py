#!/opt/homebrew/bin/python3
"""Exact additive test custody; never executes Rust or writes endpoints."""
import pathlib,json,hashlib,subprocess,re
E=pathlib.Path(__file__).resolve().parent;R=E.parents[2];D=json.loads((E/'source-bindings.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
before=(E/D['before_archive']).read_bytes();assert sha(before)==D['before_sha256'];assert before==subprocess.check_output(['git','show',D['base']+':'+D['source']],cwd=R)
def inverse(current):
    assert sha(current)==D['current_sha256'];assert current.startswith(before)
    assert sha(current[D['before_length']:])==D['complete_added_body_sha256']
    assert current[:D['before_length']]==before
    return before
current=(R/D['source']).read_bytes();inverse(current);negative=0
for wrong in [b'',current+b' foreign\n',current+current,current.replace(b'full_query { HEADER }',b'full_query { 0usize }',1)]:
    assert wrong!=current
    try:inverse(wrong)
    except AssertionError:negative+=1
    else:raise AssertionError('changed whole input admitted')
for row in D['unchanged']:assert sha((R/row['path']).read_bytes())==row['sha256']
text=current[len(before):].decode()
assert text.count('#[test]')==1 and D['selector']['selector'].split('::')[-1] in text
for token in ['reserve_skeleton(function, 1)','for full_query in [false, true]','path_array::capacity_controls()','if full_query { HEADER } else { 0 }','set_active_floor(expected_floor)','super::push!','super::optimized::push!','all six original Diagnostic fields','first.requested <= first.remaining','old_first.requested <= old_first.remaining','allocation.allocated, allocation.freed','old_allocation.allocated, old_allocation.freed']:
    assert token in text,token
assert len(current.splitlines())<=1500
native=R/D['unchanged'][-1]['path'];assert len(native.read_text().splitlines())==1500
print(json.dumps({'source_only':'PASS','full_before_and_exact_added_body':True,'original_test_assertions_preserved':len(re.findall(r'\bassert(?:_eq|_ne)?!',before.decode())),'whole_input_mutations_refused':negative,'unchanged_production_and_custody_inputs':len(D['unchanged']),'new_Rust_module_paths':0,'native_inventory_lines':1500,'Rust_execution':'NOT RUN'},sort_keys=True))
