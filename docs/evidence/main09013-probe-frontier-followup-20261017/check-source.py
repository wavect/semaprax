from pathlib import Path
import hashlib,json,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest();rows=json.loads((D/'full-source-inverses.json').read_bytes());refused=0
for row in rows:
    raw=(R/row['path']).read_bytes();old=(R/row['archive']).read_bytes()
    def restore(raw):
        assert len(raw)==row['current_length'] and sha(raw)==row['current_sha256']
        for e in row['inverse']:
            i=e['start_byte'];v=e['after'].encode();assert raw[i:i+len(v)]==v;raw=raw[:i]+e['before'].encode()+raw[i+len(v):]
        assert raw==old and len(raw)==row['before_length'] and sha(raw)==row['before_sha256'];return raw
    assert restore(raw)==subprocess.check_output(['git','show',row['before_head']+':'+row['path']],cwd=R)
    for bad in [raw+b' ',raw[:-1],b'foreign',old]:
        try:restore(bad)
        except AssertionError:refused+=1
        else:raise AssertionError('foreign stage accepted')
    assert len(raw.splitlines())<=1500
    if row['path'].endswith('tests.rs'):assert raw.startswith(old)
root='src/cleanup_plan/replay.rs';assert (R/root).read_bytes()==subprocess.check_output(['git','show',rows[0]['before_head']+':'+root],cwd=R)
helper=(R/rows[0]['path']).read_text();assert '16 * size_of::<&Vec<std::rc::Rc<SkeletonObservation>>>() // eight len/capacity actual/formal pairs' in helper
body=helper.split("pub(super) fn can_take(input:",1)[1].split('\n#[cfg(test)]\npub(super) fn note_take',1)[0]
assert body.count('input.values.len()')+body.count('input.values.capacity()')==8
print(json.dumps({'status':'SOURCE_ONLY_NOT_EXECUTED','whole609_preimages_and_inverses':2,'old_test_prefix_byteexact':True,'replay_unchanged':True,'actual_values_method_sites':8,'receiving_reference_pairs':8,'hostile_source_refusals':refused,'rust_tests_executed':False}))
