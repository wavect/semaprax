"""Finite source proof only: no Rust, binder, refresh, or production execution."""
from pathlib import Path
import ast,hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;sha=lambda b:hashlib.sha256(b).hexdigest();BASE='14d2179e11b0a2aabb2ae33b46a8591c2d8cd37f'
def git(path,ref=BASE):return subprocess.check_output(['git','show',ref+':'+path],cwd=R)
rows=json.loads((D/'whole-inverses.json').read_bytes());assert len(rows)==5
for row in rows:
 after=(R/row['path']).read_bytes();before=(D/row['before_archive']).read_bytes();assert sha(after)==row['after_sha256'] and sha(before)==row['before_sha256'];assert before==git(row['path'])
 assert row['inverse']==[dict(after=after.decode(),before=before.decode())]
owner='src/cleanup_plan/replay/ordinary_plan_history.rs';old=git(owner).decode();new=(R/owner).read_text();start=old.index('mod producer;\n');end=old.index('\nmacro_rules! paid',start);block=old[start:end];assert block.encode()==b''.join(block.encode().splitlines(True))
assert new.count(block)==1 and new.index(block)>new.index('macro_rules! history_path') and new.index(block)<new.index('\nstruct Segment {')
assert new.replace('\n'+block,'',1)==old[:start]+old[end:]
# Old production algorithm and frozen historical namespace evidence stay byte exact.
for path in ['src/cleanup_plan/replay/ordinary_plan_history/before.rs','docs/evidence/ordinary-plan-history-c8740045-v1/inputs.json','docs/evidence/ordinary-plan-history-c8740045-v1/root-inverse.json','docs/evidence/ordinary-plan-integrated-e24-v1/stages.json','docs/evidence/ordinary-plan-final-ingress-20261017/recipe.json','docs/evidence/ordinary-plan-final-ingress-20261017/helper.before-binding.rs.txt']:
 assert (R/path).read_bytes()==git(path)
flat=(R/'src/cleanup_plan/replay/ordinary_plan_history/before.rs').read_bytes();assert len(flat)==27915 and sha(flat)=='9e07f08634f9a79e9d38955da089790acf951521362e46ba23b073e67e9ba3a5'
root_old=(R/'docs/evidence/ordinary-plan-history-c8740045-v1/replay.rs.before.txt').read_text();body=root_old.split('fn plan_skeleton_paths(')[1].split('\nfn storage_regions(')[0];assert body in flat.decode()
root_inverse=json.loads((R/'docs/evidence/ordinary-plan-history-c8740045-v1/root-inverse.json').read_bytes());stages=json.loads((R/'docs/evidence/ordinary-plan-integrated-e24-v1/stages.json').read_bytes());assert stages[0]['current_sha256']==root_inverse['after_sha256']
# The complete old55-line test is a lexical subsequence; no assertion was removed.
p='src/cleanup_plan/replay/ordinary_plan_history/source_custody.rs';old=git(p).decode().splitlines(True);new=(R/p).read_text().splitlines(True);at=0
for line in old:
 while at<len(new) and new[at]!=line:at+=1
 assert at<len(new),('original test line removed',line);at+=1
# Exercise only the finite binder template function and unchanged strict scrub guard.
binder=R/'docs/evidence/ordinary-plan-final-ingress-20261017/bind-current-inputs.py';tree=ast.parse(binder.read_text());wanted={'custody_source_template','scrub'};functions=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in wanted]
def read(path,length=None,digest=None):
 b=Path(path).read_bytes();assert len(b)<=2*1024*1024
 if length is not None:assert len(b)==length
 if digest is not None:assert sha(b)==digest
 return b
ns={'read':read,'D':binder.parent,'re':re};exec(compile(ast.Module(body=functions,type_ignores=[]),str(binder),'exec'),ns)
recipe=json.loads((binder.parent/'recipe.json').read_bytes());template=(R/recipe['helper_template']).read_text();composed=ns['custody_source_template'](template);helper=(R/recipe['helper']).read_text();assert ns['scrub'](composed)==ns['scrub'](helper)
assert helper.endswith((D/'helper-addition.rs.txt').read_text());negative=0
for bad in [helper+'\n',helper.replace('27915','27916',1),helper.replace('complete current compiled flat predecessor','changed',1),helper.replace('legacy[0]["before_sha256"]','legacy[0]["after_sha256"]',1)]:
 assert ns['scrub'](bad)!=ns['scrub'](composed);negative+=1
# Binder retains its complete old source after removing the exact hook and function.
old_binder=(D/'bind-current-inputs.py.before.txt').read_text();code=binder.read_text();function=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='custody_source_template');lines=code.splitlines(True);prefix=''.join(lines[:function.lineno-1]);suffix=''.join(lines[function.end_lineno:]);restored=(prefix+suffix).replace('\n\ndef main():','\ndef main():',1).replace(' template=custody_source_template(template)\n','',1);assert restored==old_binder
# None of these edits invalidates the already one-shot MAIN090 physical binding.
physical={r['path'] for r in json.loads((R/'docs/evidence/main09013-source-namespaces-v1/input-domains.json').read_bytes())['physical']};assert not physical&{r['path'] for r in rows}
for row in rows:
 if row['path'].endswith('.rs'):assert len((R/row['path']).read_text().splitlines())<=1500
result=dict(status='PASS_SOURCE_ONLY_PLAN_MACRO_CUSTODY_FIX',base=BASE,whole_before_inverse_records=5,original_test_lines_retained=55,flat_old_owner_length=len(flat),flat_old_owner_sha256=sha(flat),unchanged_old_producer_body_bytes=len(body.encode()),finite_binder_drift_negatives=negative,MAIN090_physical_intersection=[],FINAL_six_root_rebind_required=['src/source_reflows_merge.rs'],Rust_Cargo_runtime_binder_or_refresh_executed=False)
(D/'proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
