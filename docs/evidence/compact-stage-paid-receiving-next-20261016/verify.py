#!/usr/bin/env python3
import hashlib,json,pathlib,subprocess,re
root=pathlib.Path(__file__).resolve().parents[3];packet=pathlib.Path(__file__).resolve().parent
m=json.loads((packet/'source-inverses.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
for row in m['rows']:
    current=(root/row['path']).read_bytes();before=(packet/'before'/row['path']).read_bytes()
    assert sha(current)==row['after_sha256'] and sha(before)==row['before_sha256']
    assert before==subprocess.check_output(['git','show',m['base']+':'+row['path']],cwd=root)
    restored=current.decode()
    for e in reversed(row['edits']):
        assert restored.count(e['after'])==1
        restored=restored.replace(e['after'],e['before'],1)
    assert restored.encode()==before
    if row['path'].endswith('/tests.rs'):
        old=before.decode();new=current.decode()
        for match in re.finditer(r'\bassert(?:_eq|_ne)?!\(',old):
            start=match.start();end=match.end();level=1
            while level:
                if old[end]=='(':level+=1
                elif old[end]==')':level-=1
                end+=1
            assert old[start:end] in new
s=(root/'src/cleanup_plan/replay/compact_stage.rs').read_text()
assert 'use super::*;' not in s
assert '#[cfg(test)]\nimpl Residual' not in s
assert 'source: StagedCopyResultSource' in s
oracle=(root/'src/cleanup_plan/replay/factored/oracle_order.rs').read_text()
assert 'staged(a.canonical(), b.canonical(), function, budget)' not in oracle
assert 'canonical_staged!(a, b, function, budget)' in oracle
# The lexical comparator keeps the full original field order/work after its
# original first kind charge is moved before the new metadata frontier.
def function_body(text, name):
    start=text.index(name);begin=text.index('{',start);end=begin+1;level=1
    while level:
        if text[end]=='{':level+=1
        elif text[end]=='}':level-=1
        end+=1
    return text[begin+1:end-1]
old_oracle=(packet/'before/src/cleanup_plan/replay/factored/oracle_order.rs').read_text()
original=function_body(old_oracle,'pub(super) fn staged(')
original=original.replace('    budget.charge(function, 1, "oracle staged source kind")?;\n','',1)
start=original.index('    macro_rules! ids {');end=original.index('    match (a, b)',start)
original=original[:start]+original[end:]
def expand(match):
    return '\n'.join('decide!(text('+piece.strip().replace(',', '.as_str(),',1)+'.as_str(), function, budget)?);' for piece in match.group(1).split(';') if piece.strip())
original=re.sub(r'ids!\(([^)]*)\);',expand,original)
macro=oracle[oracle.index('macro_rules! canonical_staged'):oracle.index('fn compact_staged(')]
macro=macro[macro.index('=> {{')+5:macro.rindex('}};')]
for word in ['a','b','function','budget']:
    macro=macro.replace('$'+word,word)
assert ''.join(original.split())==''.join(macro.split())
print('PASS six complete Main source inverses, every original assertion, complete lexical canonical comparison, explicit imports')
