"""Source-only exact acquisition controls; no refresh/compiler/runtime."""
from pathlib import Path
import hashlib,json,runpy,copy,ast
R=Path(__file__).resolve().parents[3]
D=R/'docs/evidence/main043-independent-custody-20261016'
P=Path(__file__).resolve().parent
sha=lambda s:hashlib.sha256(s.encode()).hexdigest()
def refused(f):
 try:f()
 except (AssertionError,KeyError,ValueError):return True
 return False
outer=runpy.run_path(str(D/'semantic-receiver-outer.py'))
view=runpy.run_path(str(D/'semantic-receiver-view.py'))
raw=(P/'current.rs.txt').read_text();prior=(P/'prior-stage.rs.txt').read_text()
oldrow=json.loads((D/'semantic-receiver-historical-view/current-to-main043.json').read_text())
assert prior==oldrow['current'] and sha(prior)==oldrow['current_sha256']
old=runpy.run_path(str(P/'docs__evidence__main043-independent-custody-20261016__semantic-receiver-view.py.before'))
assert outer['project'](raw)==prior
assert view['restore'](oldrow,raw)==old['restore'](oldrow,prior)
assert view['historical'](R,raw)==(D/'semantic-receiver-historical-view/main043-semantic_receivers.rs.txt').read_text()
negatives=0
for bad in ['',raw+' ',raw+raw,'!'+raw[1:],prior,'foreign']:
 assert refused(lambda:outer['project'](bad));negatives+=1
assert refused(lambda:outer['restore'](outer['ROW'],'foreign',raw));negatives+=1
for field in ['current_sha256','previous_sha256','previous']:
 row=copy.deepcopy(outer['ROW']);row[field]='foreign'
 assert refused(lambda:outer['restore'](row,outer['SOURCE'],raw));negatives+=1
row=copy.deepcopy(outer['ROW']);row['inverse'].reverse()
assert refused(lambda:outer['restore'](row,outer['SOURCE'],raw));negatives+=1
row=copy.deepcopy(outer['ROW']);row['inverse'][0]['previous']='foreign'
assert refused(lambda:outer['restore'](row,outer['SOURCE'],raw));negatives+=1
assert refused(lambda:json.loads('{"x":1,"x":2}',object_pairs_hook=outer['unique']));negatives+=1
for field in ['current_sha256','historical_sha256']:
 row=dict(oldrow);row[field]='0'*64
 assert refused(lambda:view['restore'](row,raw));negatives+=1
rows=json.loads((P/'source-inverses.json').read_text())
for row in rows['changed']:
 text=(R/row['path']).read_text();assert sha(text)==row['current_sha256']
 for before,after in reversed(row['edits']):
  assert text.count(after)==1;text=text.replace(after,before,1)
 assert text==row['before_source'] and sha(text)==row['before_sha256']
 if row['path'].endswith('.py'):ast.parse((R/row['path']).read_text())
module=(R/'src/source_reflows_merge/main67107_inputs.rs').read_text();before=(P/'main67107_inputs.rs.before').read_text()
assert module.startswith(before)
runpy.run_path(str(D/'guard-extensions.py'))['verify_recipe']()
assert '"63bd9f8a4f8d19369a78798a41278e5b96ab0732361598656884efd6af159ff9"'in module
assert '"1aa0092ae0b2503a0fc398efe2dd3454f9940d052f6c3e9cf5861c57e9d05b5d"'in module
ast.parse((D/'semantic-receiver-outer.py').read_text())
print(json.dumps({'status':'PASS_TARGETED_SOURCE_ONLY','negative_controls':negatives,'whole_prior_stage':sha(prior),'whole_historical':sha(view['historical'](R,raw)),'original_scripts_whole_byte_inverses':len(rows['changed']),'all_existing_Rust_helper_bytes_preserved':True,'Rust_tests':'UNEXECUTED','common_refresh_run':False,'compiler_run':False}))
