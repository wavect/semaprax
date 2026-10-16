#!/opt/homebrew/bin/python3
"""Authenticate complete additive fixture changes; no Rust execution."""
import hashlib,json,pathlib,re,subprocess
E=pathlib.Path(__file__).resolve().parent;R=E.parents[2]
D=json.loads((E/'source-bindings.json').read_text())
def sha(b):return hashlib.sha256(b).hexdigest()
def authenticate(current,row):
    assert sha(current)==row['after_sha256']
    before=(E/row['archive']).read_bytes();assert sha(before)==row['before_sha256']
    assert before==subprocess.check_output(['git','show',D['base']+':'+row['path']],cwd=R)
    lines=before.decode().splitlines(True);offset=0
    for edit in row['additions']:
        new=edit['insert'].splitlines(True);at=edit['line']+offset
        lines[at:at]=new;offset+=len(new)
    assert ''.join(lines).encode()==current
    # Complete current-to-private inverse, with no guessed substring projection.
    lines=current.decode().splitlines(True)
    for edit in reversed(row['additions']):
        prior=sum(len(e['insert'].splitlines(True)) for e in row['additions'] if e['line']<edit['line'])
        at=edit['line']+prior;new=edit['insert'].splitlines(True)
        assert lines[at:at+len(new)]==new;del lines[at:at+len(new)]
    assert ''.join(lines).encode()==before
    return before.decode()
negative=0
for row in D['changed']:
    current=(R/row['path']).read_bytes();before=authenticate(current,row)
    for wrong in [b'',current+b' foreign\n',current+current]:
        try:authenticate(wrong,row)
        except AssertionError:negative+=1
        else:raise AssertionError('complete current mutation admitted')
    if row['path'].endswith('/tests.rs'):
        # Every complete old line/assertion remains in the inverse, not a count-only claim.
        assert len(re.findall(r'\bassert(?:_eq|_ne)?!',current.decode()))>=len(re.findall(r'\bassert(?:_eq|_ne)?!',before))
        original_assertions=len(re.findall(r'\bassert(?:_eq|_ne)?!',before))
        assert len(current.splitlines())<=1500
for row in D['unchanged']:assert sha((R/row['path']).read_bytes())==row['sha256'],row['path']
assert sha((E/D['historical_failure']['archived']).read_bytes())==D['historical_failure']['sha256']
text=(R/D['changed'][0]['path']).read_text()
assert text.count('let setup_clones = if attack == 2 { 2 } else { 1 };')==1
assert text.count('budget.reserve_skeleton(function, setup_clones).unwrap();')==1
assert text.count('current.reserve_skeleton(function, 1).unwrap();')==1
assert text.count('assert_eq!(budget.skeleton_spent, setup_clones);')==2
assert text.count('assert_eq!(current.skeleton_spent, 1);')==1
assert 'all six original zero-funded setup Diagnostic fields' in text
assert 'zero clone work cannot fabricate a physical floor refusal' in text
assert 'Err(Diagnostic::io("SPX-H006", String::new()))' in text
assert len((R/D['unchanged'][8]['path']).read_text().splitlines())==1500
print(json.dumps({'source_only':'PASS','full_preimages':len(D['changed']),'unchanged_complete_production_and_custody_inputs':len(D['unchanged']),'whole_input_rejections':negative,'original_assertions_preserved':original_assertions,'test_lines':len(text.splitlines()),'native_inventory_lines':1500,'Rust_execution':'NOT RUN'},sort_keys=True))
