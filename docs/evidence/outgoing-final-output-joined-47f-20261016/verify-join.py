#!/usr/bin/env python3
"""Complete finite source-input joins; no Rust or gain claims."""
import argparse,hashlib,json,pathlib,subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);r=pathlib.Path(ap.parse_args().repo).resolve();prefix='docs/evidence/outgoing-final-output-joined-47f-20261016/';p=r/prefix;base='47f80b0f7a06fde9b2388439a9c0bfe998a05af1';sha=lambda b:hashlib.sha256(b).hexdigest();git=lambda c,s:subprocess.check_output(['git','show',c+':'+s],cwd=r)
binding=json.loads((p/'component-input-bindings.json').read_text());fronts={}
for key,subject in [('43e',binding['43e']['subject']),('276',binding['276']['subject']),('47f',base)]:
 rows=json.loads((p/f'current-to-{key}.json').read_text());front={}
 for row in rows:
  current=(r/row['source']).read_bytes();assert sha(current)==row['current_sha256'];old=current.decode()
  for edit in row['edits']:assert old.count(edit['new'])==1;old=old.replace(edit['new'],edit['old'],1)
  old=old.encode();assert sha(old)==row['original_sha256'];assert old==git(subject,row['source'])==(r/row['original_archive']).read_bytes();front[row['source']]=old
 fronts[key]=front
 if key!='47f':
  for entry in binding[key]['complete_inputs']:
   s=entry['source'];actual=front.get(s,(r/s).read_bytes());assert sha(actual)==entry['sha256'];assert actual==git(subject,s)
assert [len(fronts[k])for k in ['43e','276','47f']]==[1,2,5]
# Shipping bodies are exact reviewed components; no combined-bank edits exist.
assert (r/'src/cleanup_plan/replay/path_join.rs').read_bytes()==git(binding['43e']['subject'],'src/cleanup_plan/replay/path_join.rs')
for s in ['src/cleanup_plan/replay.rs','src/cleanup_plan/replay/branch_clone/inline_tests.rs']:
 assert (r/s).read_bytes()==git(binding['276']['subject'],s)
# Original packet JSONs, scripts, assertions, archives and receipts are complete
# immutable Git inputs in each independent view, not rewritten or exempted.
for key,packet in [('43e','docs/evidence/final-edge-state-move-next-20261016/'),('276','docs/evidence/final-output-transfer-808-next-20261016/')]:
 for s in subprocess.check_output(['git','ls-tree','-r','--name-only',binding[key]['subject'],'--',packet],cwd=r,text=True).splitlines():assert (r/s).read_bytes()==git(binding[key]['subject'],s)
replays=json.loads((p/'component-replays.json').read_text());assert len(replays)==2
for replay in replays:
 key=replay['component'];assert replay['physical_source_subject']==binding[key]['subject'] and replay['physical_checkout_clean'];assert replay['stdout']['status']=='PASS_SOURCE_ONLY';s=replay['command'][1].split('/AI-Lang/',1)[1];assert sha(git(binding[key]['subject'],s))==replay['verifier_sha256']
owners=json.loads((p/'new-owners.json').read_text());assert len(owners)==5
for owner in owners:
 data=(r/owner['source']).read_bytes();assert sha(data)==owner['sha256'];assert len(data.splitlines())==owner['lines']<=1500
newset={x['source']for x in owners};changed=subprocess.check_output(['git','diff','--name-only',base],cwd=r,text=True).splitlines();allowed=set(fronts['47f'])|newset
for s in changed:assert s in allowed or s.startswith(prefix)or s.startswith('docs/evidence/final-edge-state-move-next-20261016/')or s.startswith('docs/evidence/final-output-transfer-808-next-20261016/'),s
assert len((r/'src/cleanup_plan/replay.rs').read_text().splitlines())<=7993
assert len((r/'src/cleanup_plan/replay/path_join.rs').read_text().splitlines())<=1500
selectors=json.loads((p/'pending-selectors.json').read_text());assert len(selectors)==4 and len({x['selector']for x in selectors})==4
print(json.dumps({'status':'PASS_SOURCE_ONLY','base':base,'complete_independent_named_input_frontiers':{'43e':1,'276':2,'47f':5},'whole_component_verifier_inputs':{k:len(v['complete_inputs'])for k,v in binding.items()},'original_verifiers_replayed_unchanged_on_complete_immutable_subjects':True,'shipping_exact_reviewed_components':True,'new_owners':owners,'new_selected_bodies':4,'protected_component_packets_byte_exact':True,'global_runtime_frontiers_audits_authority':'PENDING_CANONICAL_COMPOSITION','rust_execution':False,'quantified_gain_claim':False},indent=2))
