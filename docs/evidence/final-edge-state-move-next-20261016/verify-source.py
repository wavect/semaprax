#!/usr/bin/env python3
"""Finite source-only proof; never executes Rust or measures a gain."""
import argparse, hashlib, json, pathlib, re, subprocess
ap=argparse.ArgumentParser();ap.add_argument('--repo',required=True);r=pathlib.Path(ap.parse_args().repo).resolve()
base='b3a53367d4def76240d14dca25ee414785bed9d6';prefix='docs/evidence/final-edge-state-move-next-20261016/';p=r/prefix
sha=lambda b:hashlib.sha256(b).hexdigest()
git=lambda s:subprocess.check_output(['git','show',base+':'+s],cwd=r)
row,=json.loads((p/'current-to-b3a.json').read_text());current=(r/row['source']).read_text();assert sha(current.encode())==row['current_sha256'];prior=current
for e in row['edits']:assert prior.count(e['new'])==1;prior=prior.replace(e['new'],e['old'],1)
assert prior.encode()==git(row['source'])==(r/row['original_archive']).read_bytes();assert sha(prior.encode())==row['original_sha256']
before=prior
for reg in json.loads((p/'old-registration-adapters.json').read_text()):assert before.count(reg)==1;before=before.replace(reg,'',1)
assert before==(r/'src/cleanup_plan/replay/path_join/before_final_edge.rs').read_text()
registration='\n#[cfg(test)]\nmod before_final_edge;\n#[cfg(test)]\nmod final_edge_tests;\n';assert current.endswith(registration)
def branches(s):
 out=[];start=0
 while True:
  a=s.find('                CleanupTerminator::Branch(edges) => {',start)
  if a<0:return out
  pos=s.index('{',a);depth=0
  for m in re.finditer(r'"(?:\\.|[^"\\])*"|[{}]',s[pos:]):
   t=m.group()
   if t=='{':depth+=1
   elif t=='}':
    depth-=1
    if depth==0:
     b=pos+m.end();out.append((a,b,s[a:b]));start=b;break
  else:raise AssertionError('unclosed branch')
old_branches=branches(prior);new_branches=branches(current);assert len(old_branches)==len(new_branches)==2
old_rest=prior;new_rest=current[:-len(registration)]
for a,b,s in reversed(old_branches):old_rest=old_rest[:a]+old_rest[b:]
for a,b,s in reversed(new_branches):new_rest=new_rest[:a]+new_rest[b:]
assert old_rest==new_rest,'all grouping, validators, transitions, exits and scheduling outside Branch exact'
def tokens(s):
 s=re.sub(r'//[^\n]*','',s)
 return re.sub(r',(?=\))','',re.sub(r'\s+','',s))
for (_,_,old),(_,_,new) in zip(old_branches,new_branches):
 body=old.split('                    for edge in edges {',1)[1].rsplit('\n                    }',1)[0]
 fallback=new.split('                        for edge in edges {',1)[1].rsplit('\n                        }',1)[0]
 assert tokens(fallback)==tokens(body),'full generic fallback including string literals'
 direct=new.split('                    if edges.len() == 2 {',1)[1].split('                    } else {\n                        // Complete old fallback',1)[0]
 first,last=direct.split('                        // The earlier sibling owns its independent clone.',1)
 last=last.split('                        let edge =',1)[1];last='                        let edge ='+last
 expect_first=body.replace('plan.edges[edge.0 as usize]','plan.edges[edges[0].0 as usize]',1)
 expect_last=body.replace('plan.edges[edge.0 as usize]','plan.edges[edges[1].0 as usize]',1)
 a=expect_last.index('                        let cloned = if ');b=expect_last.index('                        let next =',a);expect_last=expect_last[:a]+expect_last[b:]
 expect_last=expect_last.replace('state_for_edge(function, cloned,','state_for_edge(function, state,',1)
 assert tokens(first)==tokens(expect_first),'first complete clone/edge remains'
 assert tokens(last)==tokens(expect_last),'final only clone producer/payment removed'
 assert new.index('budget.reserve_bytes(')<new.index('if edges.len() == 2')
 assert new.count('"renewal branch history clone"')==3
for path in ['src/cleanup_plan/replay.rs','src/cleanup_plan/replay/path_join/storage.rs','src/cleanup_plan/replay/path_join/storage/empty_renewals.rs','src/cleanup_plan/replay/path_join/boxed_states.rs','src/cleanup_plan/replay/path_join/branch_join.rs','src/cleanup_plan/replay/path_join/state_move_tests.rs','src/cleanup_plan/replay/path_join/singleton_tests.rs','src/cleanup_plan/replay/path_join/receiver_tests.rs']:
 assert (r/path).read_bytes()==git(path),path
owners=json.loads((p/'new-owners.json').read_text());assert len(owners)==2
for owner in owners:
 data=(r/owner['source']).read_bytes();assert sha(data)==owner['sha256'];assert len(data.splitlines())==owner['lines']<=1500
selectors=json.loads((p/'pending-selectors.json').read_text());assert len([x for x in selectors if x['new']])==2
for x in selectors[:2]:assert ('fn '+x['selector'].rsplit('::',1)[1]+'()') in (r/owners[1]['source']).read_text()
changed=subprocess.check_output(['git','diff','--name-only',base],cwd=r,text=True).splitlines();allowed={row['source']}|{o['source'] for o in owners};assert all(s in allowed or s.startswith(prefix) for s in changed),changed
assert len(current.splitlines())<=1500
print(json.dumps({'status':'PASS_SOURCE_ONLY','base':base,'complete_current_to_git_inverse':1,'complete_old_receiver':'registration-only adapters','unchanged_outside_two_branch_dispatches':True,'first_clone_and_generic_fallback_complete':True,'final_clone_removed_with_original_work':True,'new_initial_metadata_refusal_frontier':True,'new_owners':owners,'shipping_sha256':row['current_sha256'],'new_test_bodies':2,'rust_execution':False,'quantified_gain_claim':False,'global_front_audits_authority':'PENDING_COMPOSITION'},indent=2))
