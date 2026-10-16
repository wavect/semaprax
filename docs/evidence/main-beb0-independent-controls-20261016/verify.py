#!/usr/bin/env python3
import hashlib,json,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[3]; P=Path(__file__).resolve().parent
M='beb0a8a508b2f8f30ca5277b8f88c687d859201d'; BASE='8bb9f5b7b99e1e71a797460c46180daf716b234b'
def sha(x):return hashlib.sha256(x).hexdigest()
def blob(h,p):return subprocess.check_output(['git','show',h+':'+p],cwd=R)
rows=json.loads((P/'source-inverses.json').read_text())
for row in rows:
 current=(R/row['source']).read_bytes();old=blob(M,row['main_source'])
 assert sha(current)==row['current_sha256'] and current.decode()==row['current']
 assert sha(old)==row['main_sha256'] and old.decode()==row['main']
row=json.loads((P/'runtime-input.json').read_text());assert row==rows[0]
# Main's untouched positional inverse authenticates its COMPLETE canonical body.
restored=row['main']; inv=json.loads((R/'docs/evidence/unit-weight-closure-v1/inverse.json').read_text())
for start,new,old in reversed(inv):
 assert restored[start:start+len(new)]==new
 restored=restored[:start]+old+restored[start+len(new):]
assert restored==(R/'docs/evidence/unit-weight-closure-v1/canonical_shared.before.rs.txt').read_text()
# All five test bodies retain their full original assertions and physical scopes.
for name in ['unit_weight_tests.rs','unit_weight/tests.rs']:
 row=next(x for x in rows if x['main_source'].endswith('/'+name))
 expected=row['main'].replace('super::super::super::lexical_plan_tests::','crate::cleanup_plan::replay::lexical_plan_tests::').replace('super::super::lexical_plan_tests::','crate::cleanup_plan::replay::lexical_plan_tests::')
 if name=='unit_weight_tests.rs': expected=expected.replace('include_str!("../canonical_shared.rs").to_owned()','super::historical_main_source()')
 assert row['current']==expected
assert sum(x['current'].count('#[test]') for x in rows)==5
root='src/cleanup_plan/replay/factored/canonical_shared.rs';suffix=b'\n#[cfg(test)]\nmod main_unit_weight;\n'
assert (R/root).read_bytes()==blob(BASE,root)+suffix
# These modern production owners have no change whatsoever.
for path in ['src/cleanup_plan/replay/factored.rs','src/cleanup_plan/replay/factored/canonical_shared/ordered_weights.rs','src/cleanup_plan/replay/factored/canonical_shared/memo/retained.rs','src/cleanup_plan/replay/factored/canonical_shared/closure_receiving.rs']:
 assert (R/path).read_bytes()==blob(BASE,path)
for name in ['README.md','canonical_shared.before.rs.txt','inverse.json']:
 path='docs/evidence/unit-weight-closure-v1/'+name;assert (R/path).read_bytes()==blob(M,path)
for path in (R/'src/cleanup_plan/replay/factored/canonical_shared').glob('main_unit_weight/**/*.rs'):
 assert len(path.read_text().splitlines())<=1500
# Shared nominal Label and the only two ClassifierLedger methods used by the
# independently copied Memo retain their complete original bodies.
def item(text, needle):
 start=text.index(needle);brace=text.index('{',start);depth=1;i=brace+1
 while depth:
  depth+=(text[i]=='{')-(text[i]=='}');i+=1
 return text[start:i]
for path,needles in [
 ('src/cleanup_plan/replay/factored.rs',['enum Label {']),
 ('src/bounded_output/classifier_string.rs',['pub(crate) fn capture()','pub(crate) fn current(&self)'])]:
 old=blob(M,path).decode();current=(R/path).read_text()
 for needle in needles: assert item(old,needle)==item(current,needle)
# Fixture construction and actual-entry observer remain complete old spans.
oldtests=(P/'main-tests.before.rs.txt').read_text()
fixture=oldtests[:oldtests.index('\n#[test]')].replace('super::super::lexical_plan_tests::','crate::cleanup_plan::replay::lexical_plan_tests::')
start=oldtests.index('thread_local! {');counter=oldtests[start:oldtests.index('\n#[test]',start)]
assert (R/'src/cleanup_plan/replay/factored/canonical_shared/main_unit_weight/tests.rs').read_text()==fixture+'\n'+counter
oldroot=(P/'main-factored.before.rs.txt').read_text();compiled=(R/rows[0]['source']).read_text()
for needle in ['struct Node {','struct Automaton {','    fn node(','    fn movement(','    fn epsilon(','    fn observation(','    fn end(']:
 assert item(oldroot,needle)==item(compiled,needle)
proof={'status':'SOURCE_ONLY_PASS','base':BASE,'main':M,'whole_main_canonical_inverse':True,'complete_main_source_rows':len(rows),'main_test_bodies':5,'modern_production_unchanged':True,'runtime_executed':False}
print(json.dumps(proof,indent=2));(P/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n')
