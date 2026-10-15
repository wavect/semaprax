"""Pure source/AST adapter and planner proof. Never execute a driver module."""
import ast,copy,hashlib,json,re
from pathlib import Path
ORIGINAL_SHA='7d9eb4e4d3aab9ba94afad0d236ebb32a895eb1cd3db8e1d738438a893787888'
OLD_LABEL='explicit-batched-no-run-v1'
NEW_LABEL='explicit-batched-core-lib-unit-last-no-run-v1'
UNIT="toolchain+['cargo','test','--locked','--offline','-p','semaprax','--lib','--no-run','--jobs','1','--message-format=json']"
INSERT=' core_unit_command = '+UNIT+'\n if core_unit_command in commands:\n  commands.append(commands.pop(commands.index(core_unit_command)))\n'
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def derive(original):
 assert hashlib.sha256(original.encode()).hexdigest()==ORIGINAL_SHA
 assert original.count(' return commands\n')==1 and original.count(OLD_LABEL)==2
 result=original.replace(' return commands\n',INSERT+' return commands\n',1).replace(OLD_LABEL,NEW_LABEL)
 old='# The selected lib unit harness compiles first to expose cfg(test) errors early.\n# Production lib remains required next; remaining explicit owning targets follow.\n'
 new='# Only the exact selected core lib unit no-run command moves to last.\n# Production lib and all other original literal batches retain relative order.\n'
 assert original.count(old)==1;result=result.replace(old,new,1)
 # A complete mechanical inverse proves all source outside these edits exact.
 inverse=result.replace(INSERT+' return commands\n',' return commands\n',1).replace(NEW_LABEL,OLD_LABEL).replace(new,old,1)
 assert inverse==original
 return result

def planner(source):
 tree=ast.parse(source);fn=next(x for x in tree.body if isinstance(x,ast.FunctionDef) and x.name=='planned_commands');scope={'re':re}
 exec(compile(ast.Module(body=[fn],type_ignores=[]),'isolated-pure-planned-commands','exec'),scope)
 return scope['planned_commands']
def roles(manifest):
 result=set()
 for row in manifest['checks']:
  c=row['command'];pkg=c[c.index('-p')+1]
  if '--test' in c:result.add((pkg,'test:'+c[c.index('--test')+1]))
  elif '--bin' in c:result.add((pkg,'bin:'+c[c.index('--bin')+1]))
  elif '--lib' in c:result.add((pkg,'lib'))
  else:raise AssertionError('No literal owner')
 return result

def prove(original,derived,manifest):
 assert derived==derive(original)
 old=ast.parse(original);new=ast.parse(derived)
 oldfn=next(x for x in old.body if isinstance(x,ast.FunctionDef) and x.name=='planned_commands');newfn=next(x for x in new.body if isinstance(x,ast.FunctionDef) and x.name=='planned_commands')
 inserted=ast.parse(INSERT.lstrip().replace('\n if ','\nif ').replace('\n  commands','\n commands')).body
 assert len(inserted)==2
 # Preserve the entire planning function prefix and return, adding only exact
 # Assignment+If before the original return. Normalize only honest label strings
 # elsewhere: admission, run_phase, env checks, receipts and artifact code exact.
 assert ast.dump(ast.Module(body=newfn.body[-3:-1],type_ignores=[]))==ast.dump(ast.Module(body=inserted,type_ignores=[]))
 newfn.body=newfn.body[:-3]+newfn.body[-1:]
 class Labels(ast.NodeTransformer):
  def visit_Constant(self,node):
   if node.value==NEW_LABEL:node.value=OLD_LABEL
   return node
 new=Labels().visit(new)
 assert ast.dump(old)==ast.dump(new),'Non-planning AST drift'
 oldplan=planner(original);newplan=planner(derived);tests=[]
 for toolchain in [[],['/Users/kevin/.cargo/bin/rustup','run','1.97.1']]:
  before=oldplan(manifest,toolchain);after=newplan(manifest,toolchain)
  unit=toolchain+['cargo','test','--locked','--offline','-p','semaprax','--lib','--no-run','--jobs','1','--message-format=json']
  assert before.count(unit)==1
  assert before[0]==unit and after==before[1:]+[unit]
  assert len(before)==len(after)==8
  tests.append(dict(case='all230 bodies/16 literal roles',toolchain=toolchain,commands_before=before,commands_after=after,only_exact_core_unit_command_moved=True))
  absent=copy.deepcopy(manifest);absent['checks']=[x for x in absent['checks'] if ('semaprax','lib') not in roles({'checks':[x]})]
  assert oldplan(absent,toolchain)==newplan(absent,toolchain)
  tests.append(dict(case='no selected core unit role',toolchain=toolchain,commands_unchanged=True,count=len(oldplan(absent,toolchain))))
 for bad in [dict(checks=[dict(command=['cargo','test','-p','semaprax*','--lib'])]),dict(checks=[dict(command=['cargo','test','-p','semaprax','--test','*'])])]:
  for fn in [oldplan,newplan]:
   try:fn(bad,[])
   except ValueError:pass
   else:raise AssertionError('Lost literal selector refusal')
 return dict(status='PURE_AST_PLANNER_PROOF_NO_DRIVER_OR_HEAVY_EXECUTION',selected_bodies=len(manifest['checks']),literal_roles=len(roles(manifest)),original_sha256=hashlib.sha256(original.encode()).hexdigest(),derived_sha256=hashlib.sha256(derived.encode()).hexdigest(),whole_source_inverse_exact=True,entire_other_ast_exact_after_only_receipt_label_normalization=True,original_command_batches=8,derived_command_batches=8,parent_required_rss_limit_gib=7,driver_default_and_global9gib_guard_unchanged=True,tests=tests,literal_refusal_cases=2,executed_build_commands=0,executed_test_bodies=0)
