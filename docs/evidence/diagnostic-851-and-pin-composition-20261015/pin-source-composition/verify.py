"""Source-only replay of preserved historical d4 plan; no pin/producer execution."""
from pathlib import Path
import ast,copy,json,hashlib,subprocess
from source_inverse_composition import blob,sha,compose,project,forward,undo
D=Path(__file__).resolve().parent
R=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang')
E=Path('/Volumes/Semaprax/provenance/d4cab-pin-evidence-20261015')
P=json.loads((E/'readonly-plan.json').read_text());head=P['capture_head'];checkpoint='39130d9237121b61e40b47f95c072c09a456554e';tablepath='docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json'
C=json.loads((D/'tool-change.json').read_text());old=(D/'archive-actual-and-build-patch.before.py').read_bytes();new=(D/'archive-actual-and-build-patch.py').read_bytes()
assert sha(old)==C['original_sha256'] and sha(new)==C['successor_sha256']
assert old.decode().count(C['replace_before'])==1 and old.decode().replace(C['replace_before'],C['replace_after'],1)==new.decode()
assert new.decode().replace(C['replace_after'],C['replace_before'],1)==old.decode()
for f in ['archive-actual-and-build-patch.py','source_inverse_composition.py']:ast.parse((D/f).read_text())
before={p:blob(R,head,p) for p in P['source_sha256']};candidate=before.copy()
for p,b in before.items():assert 'sha256:'+sha(b)==P['source_sha256'][p]
for e in P['edits']:
 a=e['before'].encode();b=e['after'].encode();assert candidate[e['path']].count(a)==1;candidate[e['path']]=candidate[e['path']].replace(a,b,1)
for p,b in candidate.items():assert 'sha256:'+sha(b)==P['after_sha256'][p]
stage,table,archives=compose(R,head,checkpoint,tablepath,candidate)
assert table==blob(R,head,tablepath)
legacy=json.loads(table)
negatives=[]
def refuses(label,fn):
 try:fn()
 except (ValueError,AssertionError):negatives.append(label)
 else:raise AssertionError('tamper admitted: '+label)
for row,prior in zip(stage['rows'],legacy):
 original=archives[str(Path(tablepath).parent/prior['archive'])]
 assert undo(project(row['current'].encode(),row),prior,original)==original
for label,mutate in [('complete_current_tamper',lambda r:r['current'].encode()+b' '),('wrong_raw_domain',lambda r:blob(R,head,r['path']))]:
 row=stage['rows'][0];refuses(label,lambda:project(mutate(row),row))
bad=copy.deepcopy(stage['rows'][0]);bad['checkpoint']+=' ';refuses('checkpoint_tamper',lambda:project(bad['current'].encode(),bad))
prior=copy.deepcopy(legacy[0]);prior['edits'][0]['before']+=' forbidden';archive=archives[str(Path(tablepath).parent/prior['archive'])];refuses('legacy_hook_tamper',lambda:forward(archive,prior))
refuses('archive_tamper',lambda:forward(archive+b' ',legacy[0]))
(D/'historical-d4-complete-source-projections.json').write_text(json.dumps(stage,indent=2)+'\n')
result=dict(status='SOURCE_ONLY_PASS_NOT_CURRENT_OR_PIN_QUALIFICATION',capture_head=head,checkpoint_head=checkpoint,observed_checkout_head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip(),source_projection_rows=len(stage['rows']),legacy_inverse_counts=[len(r['edits']) for r in legacy],old_table_sha256=sha(table),old_archives_unchanged=True,old_tool_whole_inverse=True,all_other_old_tool_bytes_exact=True,tamper_refusals=negatives,actual_wire_or_pin_apply_execution=False,producer_execution=False)
(D/'finite-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
