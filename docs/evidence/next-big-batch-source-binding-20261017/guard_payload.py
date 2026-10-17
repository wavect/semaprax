"""Finite whole-source admission before BOTH complete historical strict guards."""
from pathlib import Path
import hashlib,json,os,stat
R=Path(__file__).resolve().parents[3]
CONFIG={'source_packet': {'path': 'docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json', 'length': 112927, 'sha256': '336d79ff6cddc852d4750aa304b85a31494ac98c2e015bb52dfb615db7d98fe3'}, 'source_rows': 54, 'parent_guards': {'root': {'path': 'docs/evidence/earned-managed-pin42-source-coupling-20261017/guard_payload.py', 'length': 2978, 'sha256': '71b1cf1c20c2aa0d534dccfc9701a1ebe465f4e56b1f5d0eb7a344d6e1b1560f', 'archive': 'docs/evidence/next-big-batch-source-binding-20261017/guards/root.whole-before.py.txt'}, 'future': {'path': 'docs/evidence/future831-f247-final-source-binding-20261017/guard_payload.py', 'length': 10051, 'sha256': '367f3e692c69243e198a407686b85eb2871de023abb3fac366ede894f82f81aa', 'archive': 'docs/evidence/next-big-batch-source-binding-20261017/guards/future.whole-before.py.txt'}}, 'parent_sources': {'root': ['crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/workspace_graph/type_proof_tests.rs', 'tests/hir_module_boundaries.rs', 'tests/workspace/semantic_graph.rs', 'tests/workspace/semantic_graph/source_inventory.rs'], 'future': ['crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs', 'src/cleanup_plan/replay/compact_stage/main_f247_source_custody.rs', 'src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs', 'src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels/terminal_leaf.rs', 'src/cleanup_plan/replay/factored/canonical_shared/selected_labels.rs', 'src/cleanup_plan/replay/factored/canonical_shared/selected_labels/tests.rs', 'src/cleanup_plan/replay/skeleton_work/isolated_paths.rs', 'src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs', 'src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs', 'src/source_reflows/main_f247_controllers.rs', 'tests/hir_module_boundaries.rs', 'tests/workspace/semantic_graph.rs', 'tests/workspace/semantic_graph/source_inventory.rs']}, 'parent_carriers': {'root': {'tests/hir_module_boundaries.rs': (99079, '7dade8fc55a0ea5f4a16bc57d1ac8ce43ed200694a39ea6e3d33038228c8b8c1'), 'tests/workspace/semantic_graph/source_inventory.rs': (96877, '4dbeb81d84db0f80bb51ccc8fc1a25d2d71e49759ddeeccdc84c4d1748b27e11'), 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs': (157441, '369a29b5860f644b33b82e8e968ab66225faf70cfb930757aff04ba591824d4e')}, 'future': {'tests/hir_module_boundaries.rs': (99902, '46e5fbd03a01ef804e351ffe5c4ee15d61336f7ca0ab1a612839baa723f85d15'), 'tests/workspace/semantic_graph/source_inventory.rs': (97691, 'f42216ee8017033d39f6f29648613ddb4056483041ac6ed302b3db09623c3359'), 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs': (158336, '1593db2b5b16f88c509952eddf075e1b574a8c6352382bff4ca19e2478ea7517')}}, 'parent_owner': {'root': (57426, 'eba36d8815d2fd4f0e94995357e42c96085fdce1005519bca65feb4d693b5b79'), 'future': (57427, '1849b6be08210ee81f6ca1518cd51b8ccd5ff137c04781d6498be159ed3f285f')}, 'workspace_owner': 'tests/workspace/semantic_graph.rs', 'carriers': {'tests/hir_module_boundaries.rs': {'length': 100796, 'sha256': 'e3dad40652ba74ba2737e21b493ffb48787eb576974609f62f3f62ad690f443b', 'base': '../'}, 'tests/workspace/semantic_graph/source_inventory.rs': {'length': 98621, 'sha256': '4cf1b2596f589f53fe4dec8ac692738eecd04e7e3909c846a1c762d04717834c', 'base': '../../'}, 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs': {'length': 159374, 'sha256': '700318b054f35e3dbf69cf25d7a83cd3bf65d04f5674327bcbbd5af02bbdec66', 'base': '../../../../../'}}, 'required_modules': {'src/cleanup_plan/replay/branch_clone/direct.rs': {'length': 13556, 'sha256': '5f43434478af597206ee62ea0ca626c60b5c37d3fa5df1904676eb919a60c975'}, 'src/cleanup_plan/replay/branch_clone/direct/before.rs': {'length': 33355, 'sha256': 'b617d274c09b64c42a749953e2ed0d6a608d08a8ffbcfda0ce4eb9a74fcd2036'}, 'src/cleanup_plan/replay/branch_clone/direct/tests.rs': {'length': 17883, 'sha256': '7db9c82810929bcb0ac09eb1eabef4e1aa2292c783ff393178b92731e48aaff3'}, 'src/cleanup_plan/replay/compact_stage/main_f247_source_custody.rs': {'length': 15559, 'sha256': '825671ce87b04e06fdf31464bb474120016b52ae02100c4fd14345831928cb21'}, 'src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs': {'length': 63367, 'sha256': '553179e57cccd27eae0d228ad54b4717ec743007d9ae354d31ed6174266732be'}, 'src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels/terminal_leaf.rs': {'length': 7473, 'sha256': 'd611a058bf45d4c5ff05135c9c76119b366877f0c5a7ca840a205547a8f3650e'}, 'src/cleanup_plan/replay/factored/canonical_shared/selected_labels.rs': {'length': 15707, 'sha256': 'ea3b9f8e55a3f183ba7f6f07a01d6a1d71364a4ce6e246934de95e3603535edd'}, 'src/cleanup_plan/replay/factored/canonical_shared/selected_labels/tests.rs': {'length': 17690, 'sha256': '50e4d6dc7bfd923acba894af0d8bb004c97ea56bb6c23164f80e33f2c78f8ea1'}, 'src/cleanup_plan/replay/ordinary_plan_history/before_comparison.rs': {'length': 10346, 'sha256': 'e00b63474866f90d3bb537cd7587c2cb8751b384c9224782bf873860476534b7'}, 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs': {'length': 15025, 'sha256': '35b9565f8dbe7dc67164bc87540f4f49e9bf46e3dae7bb791dd3ba1b31c0ef78'}, 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs': {'length': 12288, 'sha256': '8bab706f06ddf65a9f439c4dfaa5e1c1d2d828c5af8e9df446686f15e6d58ab5'}, 'src/cleanup_plan/replay/skeleton_work/isolated_paths.rs': {'length': 2063, 'sha256': '5cc0e767a4e5e61ae5135d4193b831d941b90b6f676f6620d64fb9ac016c0d72'}, 'src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs': {'length': 11699, 'sha256': '021a0dd84f3275b7cbd69936cb8def96e5172fb39ceaa1b4b385d3962f6792f2'}, 'src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs': {'length': 7000, 'sha256': '7e3eb4e7868f45d04e6f1abd69ee35a56f32c0c355154d2e44386e2be1bc96c4'}, 'src/source_reflows/main99f_controllers.rs': {'length': 61825, 'sha256': 'b29e9e1619e914c241bfbb1d0641da61b4422b56fea574ddd8135f4717c739d7'}, 'src/source_reflows/main_f247_controllers.rs': {'length': 58236, 'sha256': 'df7852aac1d8938f364801cada6594344a75d37167e5f3512681f9ce14b3c96a'}, 'src/source_reflows_merge/main99f_inputs.rs': {'length': 35280, 'sha256': 'f51b7d60ffff5b9403650cd0dfbe42f4dc1994dc8ae05738973ce3a2456a1c7b'}, 'src/source_reflows_merge/next_big_batch_inputs.rs': {'length': 16397, 'sha256': '6e59808e53c27ac7d60543c44d70b356f735bec892bca02947a391883ff12c01'}, 'src/workspace_graph/tests/core_attempt_trace.rs': {'length': 2681, 'sha256': '7a67363c8fcb741714c5456fdbe3a10eb27c121d5951eca374adb33b4628e1e0'}, 'tests/wasm/scalar_browser_ci_contract/before_3e66.rs': {'length': 5295, 'sha256': 'b052603b549318e45f3cae0e1f3afd1c385366dac535eb9cd1e9084d4a06cec2'}, 'tests/wasm/scalar_browser_ci_contract/incoming_main3e66.rs': {'length': 7737, 'sha256': '35655ef91e476af9fa6903d00cbe9aee4da48ebcdb90b4dbcee2742fabbbb1fa'}}, 'ordered_table': {'path': 'docs/evidence/next-big-batch-source-binding-20261017/current-projection-inputs.tsv', 'length': 26040, 'sha256': 'bf9d90d2e54bd7fc117b8f885f94fba383d74207e8833a62c8c3eaa7adeeb190', 'rows': 426, 'insertions': [{'at': 24, 'path': 'src/cleanup_plan/replay/branch_clone/direct.rs'}, {'at': 25, 'path': 'src/cleanup_plan/replay/branch_clone/direct/before.rs'}, {'at': 26, 'path': 'src/cleanup_plan/replay/branch_clone/direct/tests.rs'}, {'at': 27, 'path': 'src/cleanup_plan/replay/ordinary_plan_history/before_comparison.rs'}, {'at': 28, 'path': 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs'}, {'at': 29, 'path': 'src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs'}, {'at': 30, 'path': 'src/source_reflows/main99f_controllers.rs'}, {'at': 31, 'path': 'src/source_reflows_merge/main99f_inputs.rs'}, {'at': 32, 'path': 'src/source_reflows_merge/next_big_batch_inputs.rs'}, {'at': 33, 'path': 'src/workspace_graph/tests/core_attempt_trace.rs'}, {'at': 34, 'path': 'tests/wasm/scalar_browser_ci_contract/before_3e66.rs'}, {'at': 35, 'path': 'tests/wasm/scalar_browser_ci_contract/incoming_main3e66.rs'}], 'prior': {'length': 25386, 'sha256': '2313237e3cd870841a2f5364a42676b2ed88544610ed062c6bbbab5799398d2c', 'archive': 'docs/evidence/next-big-batch-source-binding-20261017/guards/complete-prior414.tsv.txt'}}, 'public_before': {'length': 2717, 'sha256': '8f1da275939c53ce654e8d19369577d3d220403682ff9e7fc889e92cd3abe0d5', 'archive': 'docs/evidence/next-big-batch-source-binding-20261017/guards/public.whole-before.py.txt'}, 'public_path': 'docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py', 'public_substitutions': [{'before': 'docs/evidence/main043-definition-accepted-generation-20261017/definition_payload.py', 'after': 'docs/evidence/next-big-batch-source-binding-20261017/definition_adapter.py', 'count': 1}, {'before': 'raw=held(helper,3228)', 'after': 'raw=held(helper,1766)', 'count': 1}, {'before': 'assert len(raw)==3228 and sha(raw)==', 'after': 'assert len(raw)==1766 and sha(raw)==', 'count': 1}, {'before': '59e731713eea180fec3a07c7eb818633591ed948cf4462c5796dcd1810ec36f3', 'after': 'ab6e2d1c227dee30da804701dd3d5f54c57c853c08ce423f98359a8420cf2731', 'count': 1}, {'before': 'docs/evidence/earned-managed-pin42-source-coupling-20261017/guard_payload.py', 'after': 'docs/evidence/next-big-batch-source-binding-20261017/guard_payload.py', 'count': 1}, {'before': 'guard_raw=held(guard_helper,2978)', 'after': 'guard_raw=held(guard_helper,@GUARD_LENGTH@)', 'count': 1}, {'before': '71b1cf1c20c2aa0d534dccfc9701a1ebe465f4e56b1f5d0eb7a344d6e1b1560f', 'after': '@GUARD_SHA256@', 'count': 1}, {'before': "g=types.ModuleType('accepted_main043_strict_guard');g.__file__=str(guard_helper)", 'after': "g=types.ModuleType('accepted_main043_strict_guard');g.__file__=str(guard_helper);g._AUTHENTICATED_PROGRAM=guard_raw", 'count': 1}, {'before': "exec(compile(guard_raw,str(guard_helper),'exec'),g.__dict__)", 'after': "exec(compile(guard_raw,str(guard_helper),'exec'),g.__dict__)\nm._next_raw_source=g.source", 'count': 1}], 'this_guard': 'docs/evidence/next-big-batch-source-binding-20261017/guard_payload.py'}
CARRIER='tests/workspace/semantic_graph/source_inventory.rs'
SCRATCH=('src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch.rs','src/workspace_graph/expected_projection/private_layout/authority_invocation/scratch/tests.rs')
def bounded(path,length,digest):
 assert 0<=length<=262144 and hasattr(os,'O_NOFOLLOW')
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 identity=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode)and first.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as stream:raw=stream.read(length);assert len(raw)==length and not stream.read(1)
  assert identity(first)==identity(os.fstat(fd))==identity(os.lstat(path))
  assert hashlib.sha256(raw).hexdigest()==digest;raw.decode('utf8');return raw
 finally:os.close(fd)
def read(row):return bounded(R/row['archive'],row['length'],row['sha256'])
def packet():
 row=CONFIG['source_packet'];raw=bounded(R/row['path'],row['length'],row['sha256']);value=json.loads(raw)
 assert value['schema']=='semaprax.next-big-batch-whole-inputs.v1'
 assert len(value['rows'])==len({r['source']for r in value['rows']})==CONFIG['source_rows']
 return value

def source(parent,path,length,digest):
 assert parent in ('root','future')
 assert path in CONFIG['parent_sources'][parent],('foreign named parent acquisition',parent,path)
 rows=packet()['rows'];row=next((r for r in rows if r['source']==path),None)
 if row is None:return bounded(R/path,length,digest)
 current=row['current'];actual=bounded(R/path,current['length'],current['sha256']);assert actual==read(current)
 if (length,digest)==(current['length'],current['sha256']):return actual
 views=[v for v in row['views']if(length,digest)==(v['length'],v['sha256'])];assert views,('foreign whole parent tuple',parent,path,length,digest)
 values=[read(v)for v in views];assert all(v==values[0]for v in values);return values[0]

def whole_program(name):
 row=CONFIG['parent_guards'][name];raw=bounded(R/row['path'],row['length'],row['sha256']);assert raw==read(row);return raw

def replace_exact(code,old,new,count):
 assert code.count(old)==count;changed=code.replace(old,new)
 assert changed.count(new)==count and changed.replace(new,old)==code
 return changed

def root_module():
 raw=whole_program('root');code=raw.decode()
 code=replace_exact(code,"actual=bounded(R/row['source'],current['length'],current['sha256'])","actual=_next_source('root',row['source'],current['length'],current['sha256'])",1)
 old=";exec(compile(current,ns['__file__'],'exec'),ns)"
 new=";ns['_next_execute5028']=_next_execute5028;current=_next_compile3(current);exec(compile(current,ns['__file__'],'exec'),ns)"
 code=replace_exact(code,old,new,1)
 def prepare_compile3(program):
  old="exec(compile(current,ns['__file__'],'exec'),ns)";new='_next_execute5028(current,ns)'
  return replace_exact(program,old,new,1)
 def execute5028(program,namespace):
  current=replace_exact(program,'bounded(R/CARRIER,*CURRENT)','_next_root_carrier()',1)
  namespace['_next_root_carrier']=lambda:source('root',CARRIER,*CONFIG['parent_carriers']['root'][CARRIER])
  exec(compile(current,namespace['__file__'],'exec'),namespace)
 ns=dict(__file__=str(R/CONFIG['parent_guards']['root']['path']),_next_source=source,_next_compile3=prepare_compile3,_next_execute5028=execute5028)
 exec(compile(code,ns['__file__'],'exec'),ns)
 return ns['accepted_guard_extensions'](R)

def future_module():
 raw=whole_program('future');code=raw.decode()
 for old,new,count in [
  ("bounded(R/path,*row['current'])","_next_source('future',path,*row['current'])",2),
  ("bounded(R/owner,*pair)","_next_source('future',owner,*pair)",2),
  ("owner_current=bounded(R/OWNER['path'],*OWNER['current'])","owner_current=_next_source('future',OWNER['path'],*OWNER['current'])",1),
 ]:code=replace_exact(code,old,new,count)
 ns=dict(__file__=str(R/CONFIG['parent_guards']['future']['path']),_next_source=source)
 exec(compile(code,ns['__file__'],'exec'),ns)
 return ns['accepted_guard_extensions'](R)

def authenticate_current(path,supplied):
 assert path in CONFIG['carriers']
 row=CONFIG['carriers'][path];actual=bounded(R/path,row['length'],row['sha256']).decode();assert supplied==actual,'unprojected complete current carrier required'
 for owner,pin in CONFIG['required_modules'].items():bounded(R/owner,pin['length'],pin['sha256'])
 table=CONFIG['ordered_table'];raw=bounded(R/table['path'],table['length'],table['sha256']);before=read(table['prior'])
 expected=before.decode().splitlines()
 for item in table['insertions']:
  assert item['path']not in expected and 0<=item['at']<=len(expected);expected.insert(item['at'],item['path'])
 assert raw==('\n'.join(expected)+'\n').encode()
 assert len(raw.decode().splitlines())==len(set(raw.decode().splitlines()))==table['rows']
 for owner in CONFIG['required_modules']:
  literal='include_str!("'+row['base']+owner+'")'
  assert actual.count(literal)==1,('new required/max-one literal owner',path,owner)
 # A whole source row binds the new TSV reference and the earned pin literals.
 p=CONFIG['workspace_owner'];source('root',p,*CONFIG['parent_owner']['root'])
 source('future',p,*CONFIG['parent_owner']['future'])
 return actual

def public_previous():
 row=CONFIG['public_before'];before=read(row);current=before.decode()
 program=_AUTHENTICATED_PROGRAM
 assert isinstance(program,bytes)and len(program)<=65536
 assert bounded(R/CONFIG['this_guard'],len(program),hashlib.sha256(program).hexdigest())==program
 replacements=[]
 for edit in CONFIG['public_substitutions']:
  after=edit['after'].replace('@GUARD_LENGTH@',str(len(program))).replace('@GUARD_SHA256@',hashlib.sha256(program).hexdigest())
  current=replace_exact(current,edit['before'],after,edit['count']);replacements.append((edit['before'],after,edit['count']))
 restored=current
 for old,new,count in reversed(replacements):
  assert restored.count(new)==count;restored=restored.replace(new,old)
 assert restored.encode()==before
 actual=current.encode();assert bounded(R/CONFIG['public_path'],len(actual),hashlib.sha256(actual).hexdigest())==actual
 return before.decode()

def accepted_guard_extensions(root):
 assert Path(root).resolve()==R
 parents={'root':root_module(),'future':future_module()}
 verifiers={name:m['verify_guard']for name,m in parents.items()};extractors={name:m['fragments']for name,m in parents.items()}
 def prior(name,path):return source(name,path,*CONFIG['parent_carriers'][name][path]).decode()
 def verify_guard(path,current,original,refresh=False):
  assert not refresh,'all original guard bindings remain frozen';authenticate_current(path,current)
  records={name:f(path,prior(name,path),original,False)for name,f in verifiers.items()}
  return records['root'] # This remains the whole historical record, never relabelled actual.
 def fragments(path,current,original):
  authenticate_current(path,current)
  values={name:f(path,prior(name,path),original)for name,f in extractors.items()}
  return values['root']
 m=dict(parents['root']);m['verify_guard']=verify_guard;m['fragments']=fragments
 # Retain the whole original recipe and assertion; only its current public
 # acquisition obtains the exact authenticated predecessor before comparison.
 old_recipe="def verify_recipe():assert(D/'verify-or-refresh.py').read_text()==recipe()"
 new_recipe='def verify_recipe():assert _next_public_previous()==recipe()'
 old_guard=bounded(R/'docs/evidence/main043-independent-custody-20261016/guard-extensions.py',5113,'ccfb62c8b30e5564446ddc6eb2a31f5a7502e5ed43d3d5077a8c0105c42ef95a').decode()
 assert old_guard.count(old_recipe)==1
 recipe_scope=dict(m);recipe_scope['_next_public_previous']=public_previous
 exec(compile(replace_exact(old_recipe,old_recipe,new_recipe,1),str(R/CONFIG['this_guard']),'exec'),recipe_scope)
 m['verify_recipe']=recipe_scope['verify_recipe'];m['public_previous']=public_previous
 return m
