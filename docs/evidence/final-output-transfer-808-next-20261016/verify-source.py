#!/usr/bin/env python3
"""Finite source-only custody; does not execute Rust or establish savings."""
import argparse, hashlib, json, re, subprocess
from pathlib import Path
arg=argparse.ArgumentParser();arg.add_argument('--repo',default='.');args=arg.parse_args()
r=Path(args.repo).resolve();p=r/'docs/evidence/final-output-transfer-808-next-20261016'
m=json.loads((p/'source-inverses.json').read_text());h=lambda b:hashlib.sha256(b).hexdigest()
for row in m['rows']:
 current=(r/row['path']).read_bytes();old=(p/row['before_archive']).read_bytes()
 assert h(current)==row['current_sha256'] and h(old)==row['before_sha256'],row['path']
 assert subprocess.check_output(['git','show',m['base']+':'+row['path']],cwd=r)==old
 lines=current.decode().splitlines(keepends=True)
 for edit in reversed(row['edits']):
  i,j=edit['current_start_line']-1,edit['current_end_line']-1
  assert ''.join(lines[i:j])==edit['current']
  lines[i:j]=[edit['old']]
 assert ''.join(lines).encode()==old,row['path']
for row in m['new_sources']:assert h((r/row['path']).read_bytes())==row['sha256'],row['path']
def func(s,name):
 a=s.index('fn '+name+'(');return s[a:s.index('\n}',a)+2]+'\n'
old=(p/'before/src__cleanup_plan__replay.rs').read_text();current=(r/'src/cleanup_plan/replay.rs').read_text()
seq=func(old,'sequence_skeleton_paths');plan=func(old,'plan_skeleton_paths')
for name,s in [('sequence_skeleton_paths',seq),('plan_skeleton_paths',plan)]:
 assert (p/(name+'.before.rs.txt')).read_text()==s
 expected='pub(in crate::cleanup_plan::replay) '+s
 assert (r/'src/cleanup_plan/replay/final_output/before.rs').read_text().count(expected)==1
 restored=func(current,name);current=current.replace(restored,s,1)
assert current.replace('mod final_output;\n','',1)==old
actual=(r/'src/cleanup_plan/replay.rs').read_text()
loop=seq[seq.index('        for suffix in suffixes.iter() {'):seq.index('\n    }\n    Ok(combined)')]
assert actual.count(loop)==1,'complete non-one suffix loop'
loop=plan[plan.index('                for edge in edges {'):plan.index('\n            }\n            CleanupTerminator::Exit')]
assert actual.count(loop)==1,'complete non-two/full-capacity loop'
# Full first-sibling producer is identical, not a reconstructed approximation.
a=plan.index('                    budget.reserve_bytes(',plan.index('let observation = match &edge.condition'))
b=plan.index('\n                }',a)
assert actual.count(plan[a:b])==2,'unchanged first sibling and complete fallback'
matcher=plan[plan.index('match &edge.condition {'):plan.index('\n                    };',plan.index('match &edge.condition {'))]+'\n                    }'
module=(r/'src/cleanup_plan/replay/final_output.rs').read_text()
a=module.index('match &$edge.condition {');b=module.index('\n    }};',a)
macro=module[a:b].replace('$crate::cleanup_plan::replay::','').replace('$edge','edge').replace('$budget','budget').replace('$function','function')
# Preserve every literal token while permitting the explicit hygiene paths and indentation.
tokens=lambda s:re.findall(r'"(?:\\.|[^"\\])*"|[A-Za-z_][A-Za-z_0-9]*|[^\s]',s)
left,right=tokens(matcher),tokens(macro)
# rustfmt removes one transparent Boolean arm block at the shallower macro
# indentation. Authenticate exactly that pair of braces; all literal tokens stay.
i=left.index('BooleanResult');a=left.index('>',i)+1
assert left[a:a+5]==['{','SkeletonObservation',':',':','Boolean']
depth=0
for b in range(a,len(left)):
 if left[b]=='{':depth+=1
 elif left[b]=='}':
  depth-=1
  if depth==0:break
left=left[:a]+left[a+1:b]+left[b+1:]
def arm_commas(values):
 return [v for i,v in enumerate(values) if not (v==',' and i>0 and i+1<len(values) and (values[i+1] in (')',']','}') or (values[i-1]=='}' and values[i+1]=='EdgeCondition')))]
assert arm_commas(left)==arm_commas(right),'complete match + explicit transparent Boolean block/optional arm comma adapters'  
inline_old=(p/'before/src__cleanup_plan__replay__branch_clone__inline_tests.rs').read_text()
inline=(r/'src/cleanup_plan/replay/branch_clone/inline_tests.rs').read_text()
assert inline==inline_old.replace('        super::super::plan_skeleton_paths(function, budget)',
 '        // Historical inline-fusion claim: the subsequent final-output transfer\n        // has its own complete shipping observer in final_output::tests.\n        super::super::final_output::before::plan_skeleton_paths(function, budget)')
for path in ['src/cleanup_plan/replay/branch_clone.rs','src/cleanup_plan/replay/skeleton_work.rs','src/cleanup_plan/replay/skeleton_work/ordinary_dag.rs','src/cleanup_plan/replay/physical_budget.rs','src/cleanup_plan/replay/physical_clone.rs','src/cleanup_plan/replay/path_array.rs','src/cleanup_plan/replay/lexical_skeleton.rs']:
 assert (r/path).read_bytes()==subprocess.check_output(['git','show',m['base']+':'+path],cwd=r),path
assert len((r/'src/cleanup_plan/replay.rs').read_text().splitlines())<=7993
for x in m['new_sources']:assert len((r/x['path']).read_text().splitlines())<=1500
# No historical evidence modification, including all runtime source tables.
for path in subprocess.check_output(['git','diff',m['base'],'--name-only','--diff-filter=M','--','docs/evidence'],cwd=r,text=True).splitlines():
 assert path.startswith('docs/evidence/final-output-transfer-808-next-20261016/'),path
print(json.dumps({'status':'PASS_SOURCE_ONLY','base':m['base'],'whole_inverse_inputs':len(m['rows']),'complete_old_functions':2,'new_sources':3,'pending_bodies':2,'generic_fallbacks_exact':True,'first_sibling_and_observation_match_exact':True,'historical_inline_body_assertions_retained':True,'no_runtime_or_gain_claim':True},indent=2))
