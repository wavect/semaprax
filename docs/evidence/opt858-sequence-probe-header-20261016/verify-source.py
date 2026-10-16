#!/opt/homebrew/bin/python3
"""Source-only whole-byte custody; deliberately never executes Rust."""
import hashlib,json,pathlib,re,subprocess
E=pathlib.Path(__file__).resolve().parent
R=E.parents[2]
M=json.loads((E/'source-bindings.json').read_text())
def digest(b): return hashlib.sha256(b).hexdigest()
def inverse(b,row):
    assert digest(b)==row['after_sha256'], row['path']
    lines=(E/row['archive']).read_text().splitlines(True)
    offset=0
    for edit in row['edits']:
        start=edit['old_start']+offset
        old=edit['old'].splitlines(True); new=edit['new'].splitlines(True)
        assert lines[start:start+len(old)]==old
        lines[start:start+len(old)]=new
        offset+=len(new)-len(old)
    assert ''.join(lines).encode()==b
    before=(E/row['archive']).read_bytes()
    assert digest(before)==row['before_sha256']
    return before
negative=0
for row in M['changed']:
    raw=(R/row['path']).read_bytes()
    before=inverse(raw,row)
    assert before==subprocess.check_output(['git','show',M['base']+':'+row['path']],cwd=R)
    for wrong in [b'',raw+b' foreign\n',raw+raw]:
        try: inverse(wrong,row)
        except AssertionError: negative+=1
        else: raise AssertionError('whole-input mutation admitted')
for row in M['new']+M['unchanged']:
    raw=(R/row['path']).read_bytes(); assert digest(raw)==row['sha256'],row['path']
for row in M['unchanged']:
    assert (R/row['path']).read_bytes()==subprocess.check_output(['git','show',M['base']+':'+row['path']],cwd=R)
def text(p):return (R/p).read_text()
def archive(p):return (E/'before'/p.replace('/','__')).read_text()
root='src/cleanup_plan/replay.rs'
owner='src/cleanup_plan/replay/path_array/sequence_minimum'
old_call='path_array::sequence_minimum::push!('
new_call='path_array::sequence_minimum::optimized::push!('
assert text(root).count(new_call)==1
assert text(root).replace(new_call,old_call,1)==archive(root)
def sequence(s):return 'fn sequence_skeleton_paths('+s.split('fn sequence_skeleton_paths(',1)[1].split('\nuse path_summary::has_active_paths;',1)[0]
original=sequence(archive(root)); current=sequence(text(root))
assert (E/'immediate-sequence.rs.before.txt').read_text()==original
immediate=text(owner+'/immediate.rs')
assert immediate.split('pub(super) ',1)[1]==original
helper=text(owner+'.rs')
for marker in ['pub(in crate::cleanup_plan::replay) fn capacity(', 'macro_rules! push {','pub(in crate::cleanup_plan::replay) use push;']:
    assert helper.count('#[cfg(test)]\n'+marker)==1
    helper=helper.replace('#[cfg(test)]\n'+marker,marker,1)
append='\npub(in crate::cleanup_plan::replay) mod optimized;\n#[cfg(test)]\nmod immediate;\n#[cfg(test)]\npub(in crate::cleanup_plan::replay) mod optimized_tests;\n'
assert helper.endswith(append); assert helper[:-len(append)]==archive(owner+'.rs')
test=text(owner+'/tests.rs')
assert test.replace('use super::immediate::sequence_skeleton_paths;\n','',1)==archive(owner+'/tests.rs')
assert len(re.findall(r'\bassert(?:_eq|_ne)?!',test))==len(re.findall(r'\bassert(?:_eq|_ne)?!',archive(owner+'/tests.rs')))
# New exact front join composes with all four immutable parent Clippy inverses.
row=json.loads((E/'sequence-input.json').read_text())
assert digest(current.encode())==row['current_sha256']
assert current.count(row['after'])==1
restored=current.replace(row['after'],row['before'],1)
assert restored==original and digest(restored.encode())==row['original_sha256']
parent=json.loads(text('docs/evidence/sequence-minimum-clippy-input-20261016/input.json'))
assert digest(restored.encode())==parent['current_sha256']
assert len(parent['edits'])==3
for edit in parent['edits']:
    assert restored.count(edit['new'])==1
    restored=restored.replace(edit['new'],edit['old'],1)
assert digest(restored.encode())==parent['original_sha256']
bridge='src/source_reflows_merge/current344.rs'
assert text(bridge).replace('    let bound = super::opt858_sequence_input::sequence(current);\n    let current = bound.as_str();\n','',1)==archive(bridge)
# Static module resolution, not an execution/compilation assertion.
for child in ['optimized','immediate','optimized_tests']:
    assert re.search(r'\bmod '+child+r';',text(owner+'.rs'))
    assert (R/(owner+'/'+child+'.rs')).is_file()
assert 'mod opt858_sequence_input;' in text('src/source_reflows_merge.rs')
assert '#[path' not in text(owner+'.rs') and '#[path' not in text('src/source_reflows_merge/opt858_sequence_input.rs')
# All subjects remain raw, once per independent complete source inventory.
guards=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']
for guard in guards:
    for subject in M['new']:
        assert text(guard).count('/'+subject['path']+'"')==1,(guard,subject['path'])
assert len(text(guards[-1]).splitlines())==1500
for subject in M['new']:
    assert len(text(subject['path']).splitlines())<=1500
allowed=json.loads(text('docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json'))
old_allowed=json.loads(archive('docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json'))
# The manifest's full inverse above authenticates every original allow-list byte.
assert allowed.keys()==old_allowed.keys()
for guard in allowed:
    assert allowed[guard].keys()==old_allowed[guard].keys()
    for policy in allowed[guard]:
        expected=dict(old_allowed[guard][policy])
        for subject in M['new']:
            assert subject['path'] not in expected
            expected[subject['path']]=1
        assert allowed[guard][policy]==expected
optimized=text(owner+'/optimized.rs')
markers=['$work.charge(1, phase)?;', 'note_skeleton_materialization();','path_array::capacity_controls(),','let capacity = match $minimum.checked_next_power_of_two()', 'sequence_minimum::optimized::HEADER,','let capacity = if $work.budget.selected()', 'let remaining = $pending.len();','let can_grow = match capacity.checked_sub($minimum)', 'sequence_minimum::RECEIVING,','sequence_minimum::optimized::capacity(','.reserve_vec($work.function, target, capacity, phase)?;', 'target.push(path);','$first = false;']
positions=[optimized.index(x) for x in markers]; assert positions==sorted(positions)
assert 'const TOTAL: usize = HEADER + super::RECEIVING;' in optimized
assert 'if bytes >= TOTAL' in optimized
assert '+ 12 * size_of::<usize>()' in optimized and '+ 8 * size_of::<bool>()' in optimized
assert 'refund' not in optimized and 'credit' not in optimized
selectors=[]
for path in [owner+'/optimized_tests.rs','src/source_reflows_merge/opt858_sequence_input.rs']:
    selectors += re.findall(r'#\[test\]\s+fn (\w+)',text(path))
assert len(selectors)==7
print(json.dumps({'source_only':'PASS','complete_preimages':len(M['changed']),'new_raw_subjects':len(M['new']),'unchanged_complete_inputs':len(M['unchanged']),'whole_input_rejection_controls':negative,'pending_new_rust_owners':len(selectors),'native_inventory_lines':1500,'execution':'NOT RUN'},sort_keys=True))
