"""Finite domain membership atop unchanged complete source-role authentication."""
from pathlib import Path
import copy,hashlib,importlib.util,json,re,subprocess
BASE='a82e00cc83f99b16249181c45710d962598577db'
DREL='docs/evidence/source-boundary-domains-20261016'
FREL='docs/evidence/a3-e47-final-custody-20261016'
def sha(b):return hashlib.sha256(b).hexdigest()
def git(repo,ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=repo)
def protected(repo):
 d=repo/DREL;j=json.loads((d/'original-tool-bindings.json').read_text())
 for row in j['tools']:
  actual=(repo/row['source']).read_bytes();assert actual==(repo/row['archive']).read_bytes() and sha(actual)==row['sha256'],row['source']
 helper=_protected_helper(repo,j);assert helper==(repo/j['helper_archive']).read_bytes() and sha(helper)==j['helper_before_sha256']
 assert helper==git(repo,j['before_head'],'tests/support/production_source_inventory.rs')
 sealed=json.loads((d/'sealed-domains.json').read_text());result={}
 for row in sealed['domains']:
  assert row['baseline']==BASE and row['name'] in ['hir','projection'];assert row['source']==FREL+'/'+row['name']+'.json';assert row['original_complete_classes']=={'hir':1046,'projection':179}[row['name']];raw=(d/row['archive']).read_bytes();assert raw==git(repo,BASE,row['source']) and len(raw)==row['length'] and sha(raw)==row['sha256']
  original=json.loads(raw);roots=set(original['classes']);assert roots==set(row['scope_roots']) and len(roots)==row['original_complete_classes'];result[row['name']]=(row,original)
 assert set(result)=={'hir','projection'};return result

def original_verifier(repo):
 original=repo/FREL/'verify-authority.py';text=original.read_text();prefix=text.split('proof={}\n',1)[0];assert len(prefix)<len(text)
 ns={'__file__':str(original)};exec(compile(prefix,str(original),'exec'),ns);ns['R']=repo;ns['D']=repo/FREL
 return ns['verify'],sha(prefix.encode())

def original_generated(repo):
 # Keep the protected original definition as the independently checked legacy
 # baseline, while ordinary current generation uses the declared successor.
 original=repo/FREL/'generate-authority.py';text=original.read_text();anchor="for name,chunks in [('hir',hir_chunks),('projection',projection_chunks)]:";prefix=text.split(anchor,1)[0];assert len(prefix)<len(text)
 successor=repo/DREL/'current_generation.py';spec=importlib.util.spec_from_file_location('semaprax_current_source_generation',successor);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
 return module.generate(repo),sha(prefix.encode())

def compiled(text):
 text=text.strip()
 if text.startswith('include_str!'):return False
 if text.startswith('include!'):return True
 assert re.search(r'\bmod\s+(?:r#)?\w+\s*[;{]\s*$',text),('unknown actual registration',text)
 return True

def membership(full,roots):
 classes=full['classes'];assert roots<=set(classes),'lost original audited source';edges={}
 for row in classes.values():
  for chain in row['chains']:
   for edge in chain['edges']:
    if compiled(edge['text']):edges.setdefault(edge['parent'],set()).add(edge['child'])
 members=set(roots);todo=list(roots)
 while todo:
  for child in edges.get(todo.pop(),set()):
   assert child in classes,('unclassified actual boundary child',child)
   if child not in members:members.add(child);todo.append(child)
 return members

def scope(full,seal):
 row,baseline=seal;roots=set(baseline['classes']);members=membership(full,roots)
 production={p for p in members if full['classes'][p]['role']=='production'}
 outside={p for p,v in full['classes'].items() if v['role']=='production' and p not in members}
 return dict(baseline=BASE,baseline_sha256=row['sha256'],original_roots=sorted(roots),current_members=sorted(members),production_members=sorted(production),fully_authenticated_outside_production=sorted(outside))

def validate_scope(full,sealed,s):
 expected=scope(full,sealed)
 for key,value in expected.items():assert s[key]==value,('domain membership/role mismatch',key)
 return set(expected['current_members'])

def boundary_text(repo,full,members):
 text={p:(repo/p).read_text() for p in full['sources']};pieces=[]
 for chunk in full['chunks']:
  if 'literal'in chunk:pieces.append(chunk['literal'])
  elif chunk['path']in members and full['classes'][chunk['path']]['role']=='production':pieces.append(text[chunk['path']])
 for path in full['closure_only']:
  if path in members and full['classes'][path]['role']=='production':pieces.append(text[path])
 return ''.join(pieces)

def forbidden(repo,name):
 owner=(repo/('tests/hir_module_boundaries.rs'if name=='hir'else'tests/workspace/semantic_graph.rs')).read_text()
 anchor=owner.index('let validation = production_source_inventory'if name=='hir'else'let authority_projection = production_projection')
 block=owner[owner.index('for forbidden in [',anchor):];block=block[:block.index('] {')];tokens=re.findall(r'"([^"\n]*)"',block)
 return (['use super::*;']if name=='projection'else[])+tokens

def generated_scope(repo,full_dir,regenerated=None):
 sealed=protected(repo);verify,verify_sha=original_verifier(repo);generated,generator_sha=regenerated if regenerated is not None else original_generated(repo);domains={};proof={}
 for name in ['hir','projection']:
  file=full_dir/(name+'.json');raw=file.read_bytes();full=json.loads(raw)
  assert full==generated[name],('full current manifest lost actual original-parser registration/include closure',name)
  whole,all_production,production,test=verify(full)
  s=scope(full,sealed[name]);s['full_manifest_sha256']=sha(raw);s['full_raw_sha256']=sha(whole.encode());s['full_authenticated_production_sha256']=sha(all_production.encode());domains[name]=s
  actual=boundary_text(repo,full,set(s['current_members']));bad=[x for x in forbidden(repo,name) if x in actual];assert not bad,(name,'actual owning boundary forbidden',bad)
  proof[name]=dict(original_roots=len(s['original_roots']),current_boundary_members=len(s['current_members']),current_boundary_production=len(s['production_members']),outside_production_still_fully_authenticated=len(s['fully_authenticated_outside_production']),full_role_production=len(production),full_role_test_only=len(test),full_raw_sha256=s['full_raw_sha256'],boundary_production_sha256=sha(actual.encode()),exact_original_forbidden_tokens=forbidden(repo,name),all_original_coverage_preserved=True,actual_current_registered_children_complete=True,global_polluted_purity_result_not_claimed=True)
 return dict(schema='semaprax.source-boundary-domains.v1',status='SOURCE_ONLY_GENERATED_NOT_RUST_QUALIFICATION',domains=domains),dict(mode='SOURCE_ONLY_NO_RUST_EXECUTION',original_generator_definition_span_sha256=generator_sha,current_generator_source_sha256=sha((repo/DREL/'current_generation.py').read_bytes()),original_verifier_definition_span_sha256=verify_sha,domains=proof)

def negative_checks(repo,full_dir,domains):
 sealed=protected(repo);verify,_=original_verifier(repo)
 for name in ['hir','projection']:
  full=json.loads((full_dir/(name+'.json')).read_text());s=domains['domains'][name];members=set(s['current_members']);roots=set(s['original_roots']);child=next(iter(sorted(members-roots)))
  broken=copy.deepcopy(s);broken['current_members'].remove(child)
  try:validate_scope(full,sealed[name],broken)
  except AssertionError:pass
  else:raise AssertionError('dropped actual current child accepted')
  # Even coordinated removal from classes cannot pass the independently
  # regenerated complete original-parser output or the sealed full-manifest hash.
  changed=copy.deepcopy(full);del changed['classes'][child];changed['closure_only']=[p for p in changed['closure_only']if p!=child]
  assert changed!=full and sha((json.dumps(changed,separators=(',',':'))+'\n').encode())!=s['full_manifest_sha256']
  production=next(p for p,v in full['classes'].items()if v['role']=='production');changed=copy.deepcopy(full);changed['classes'][production]['role']='test-only'
  try:verify(changed)
  except AssertionError:pass
  else:raise AssertionError('production reclassification accepted')
  before=(repo/production).read_bytes()
  try:verify(full,{production:before+b' '})
  except AssertionError:pass
  else:raise AssertionError('actual source drift accepted')
  raw=verify(full)[0];assert sha((raw+'\n').encode())!=s['full_raw_sha256']
  # A declared #[path] child outside its parent's directory remains included.
  parent=next(p for p in roots if full['classes'][p]['role']=='production');changed=copy.deepcopy(full);chain=copy.deepcopy(changed['classes'][parent]['chains'][0]);foreign='src/foreign_boundary_child.rs';chain['edges'].append(dict(parent=parent,child=foreign,text='#[path = "../foreign_boundary_child.rs"] mod relocated;',attributes=['#[path = "../foreign_boundary_child.rs"]'],start=0));changed['classes'][foreign]=dict(role='production',chains=[chain]);assert foreign in membership(changed,roots)
  try:validate_scope(changed,sealed[name],s)
  except AssertionError:pass
  else:raise AssertionError('foreign-path actual child escaped sealed membership')
 return dict(dropped_actual_child_refused=True,coordinated_class_drop_not_original_generated_output=True,production_reclassification_refused=True,actual_source_drift_refused=True,whole_raw_drift_refused=True,foreign_path_child_cannot_escape=True)


def _restore_protected_helper(actual):
 # Authenticate the complete current helper before selecting the unchanged
 # historical helper used by protected(). Only its complete acquisition
 # function moved; every other original byte remains in the physical helper.
 assert len(actual)==11107 and sha(actual)=='8aaa95e240657f3cc5cd97fc7034a8eadf36af8239cd5aaa8073520007653f54', 'complete current protected helper'
 current=b'fn current_source(path: &str, row: &serde_json::Value) -> String {\n    let path = Path::new(path);\n    assert!(path\n        .components()\n        .all(|part| matches!(part, Component::Normal(_))));\n    let length = usize::try_from(row["length"].as_u64().unwrap()).unwrap();\n    assert!(length <= 16 * 1024 * 1024, "bounded current source");\n    let mut file = std::fs::File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();\n    let metadata = file.metadata().unwrap();\n    assert!(metadata.is_file());\n    let interop_epoch = path == Path::new("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs");\n    let read_length = if interop_epoch {\n        assert_eq!(length, 229209);\n        assert_eq!(row["sha256"], "d1edee35f9a7a97e9478264a44b7e77e4b3903bcbb87bb327602053f22afa6c8");\n        233195\n    } else { length };\n    assert_eq!(metadata.len(), read_length as u64);\n    let mut bytes = Vec::new();\n    bytes.try_reserve_exact(read_length).unwrap();\n    bytes.resize(read_length, 0);\n    file.read_exact(&mut bytes).unwrap();\n    let mut tail = [0];\n    assert_eq!(file.read(&mut tail).unwrap(), 0);\n    if interop_epoch {\n        let compiled = include_bytes!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs");\n        assert_eq!(bytes.as_slice(), compiled.as_slice());\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "0e793e87c5913d29d090949527031177cf4ed760e1051bb1f032f21580f1782c");\n        let mut main3d81_epoch = String::from_utf8(bytes).unwrap();\n        let main3d81_raw_arm = "main3d81_before.rs\\"),include_str!(\\"../../../../../src/cleanup_plan/replay/path_join/";\n        assert_eq!(main3d81_epoch.matches(main3d81_raw_arm).count(), 1);\n        main3d81_epoch = main3d81_epoch.replacen(main3d81_raw_arm, "", 1);\n        bytes = main3d81_epoch.into_bytes();\n        assert_eq!(bytes.len(), 233111);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "efd27c7b797d0f0414bc7ae292be0c518455d6f1dbef4f849350318181b20b60");\n        let mut field_epoch = String::from_utf8(bytes).unwrap();\n        let field_raw_arms = "include_str!(\\"../../../../../src/cleanup_plan/replay/path_join/field_local/mod.rs\\"),include_str!(\\"../../../../../src/cleanup_plan/replay/path_join/field_local/frontier.rs\\"),include_str!(\\"../../../../../src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\\"),include_str!(\\"../../../../../src/cleanup_plan/replay/path_join/field_local/tests.rs\\"),";\n        assert_eq!(field_epoch.matches(field_raw_arms).count(), 1);\n        field_epoch = field_epoch.replacen(field_raw_arms, "", 1);\n        bytes = field_epoch.into_bytes();\n        assert_eq!(bytes.len(), 232761);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "0f8b4dff2deb10dc0989ddf58723fb7137442cacffc18870d9e2afe566c8d3d0");\n        let mut retained_epoch = String::from_utf8(bytes).unwrap();\n        let retained_raw_arm = ",include_str!(\\"../../../../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\\")";\n        assert_eq!(retained_epoch.matches(retained_raw_arm).count(), 1);\n        retained_epoch = retained_epoch.replacen(retained_raw_arm, "", 1);\n        bytes = retained_epoch.into_bytes();\n        assert_eq!(bytes.len(), 232648);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "208348c0444014d05129580b71eef50e9837425c6b9cb7286cf21bc90c9d1e3f");\n        let original = include_bytes!("../../e/d1edee35f9a7a97e9478264a44b7e77e4b3903bcbb87bb327602053f22afa6c8");\n        assert_eq!(original.len(), length);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(original))), row["sha256"].as_str().unwrap());\n        bytes = original.to_vec();\n        assert_eq!(std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap().as_slice(), compiled.as_slice());\n    }\n    if path == Path::new("Cargo.toml") {\n        // Acquire the complete physical/compiled dependency-update epoch first.\n        assert_eq!(bytes.as_slice(), include_bytes!("../../Cargo.toml").as_slice());\n        assert_eq!(bytes.len(), 4411);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "206f6e4ac63533b6f1e136ffe44c3d80f580a1009cc05688c771ab4ea4e5e99b");\n        let mut manifest = String::from_utf8(bytes).unwrap();\n        for (current, before) in [\n            ("wasmparser = { version = \\"=0.262.0\\", default-features = false, features = [\\"component-model\\", \\"features\\", \\"validate\\"] }\\nwasm-encoder = { version = \\"=0.262.0\\", default-features = false, features = [\\"std\\", \\"component-model\\"] }\\n", "wasmparser = { version = \\"=0.261.0\\", default-features = false, features = [\\"component-model\\", \\"features\\", \\"validate\\"] }\\nwasm-encoder = { version = \\"=0.261.0\\", default-features = false, features = [\\"std\\", \\"component-model\\"] }\\n"),\n            ("libc = \\"=0.2.191\\"\\n", "libc = \\"=0.2.190\\"\\n"),\n        ] {\n            assert_eq!(manifest.matches(current).count(), 1);\n            manifest = manifest.replacen(current, before, 1);\n        }\n        bytes = manifest.into_bytes();\n        assert_eq!(bytes.len(), length);\n        assert_eq!(std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap().as_slice(), include_bytes!("../../Cargo.toml").as_slice());\n    }\n    let digest = format!(\n        "{:x}",\n        semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))\n    );\n    assert_eq!(\n        digest,\n        row["sha256"].as_str().unwrap(),\n        "actual current source drift: {}",\n        path.display()\n    );\n    String::from_utf8(bytes).unwrap()\n}'
 original=b'fn current_source(path: &str, row: &serde_json::Value) -> String {\n    let path = Path::new(path);\n    assert!(path\n        .components()\n        .all(|part| matches!(part, Component::Normal(_))));\n    let length = usize::try_from(row["length"].as_u64().unwrap()).unwrap();\n    assert!(length <= 16 * 1024 * 1024, "bounded current source");\n    let mut file = std::fs::File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();\n    let metadata = file.metadata().unwrap();\n    assert!(metadata.is_file());\n    assert_eq!(metadata.len(), length as u64);\n    let mut bytes = Vec::new();\n    bytes.try_reserve_exact(length).unwrap();\n    bytes.resize(length, 0);\n    file.read_exact(&mut bytes).unwrap();\n    let mut tail = [0];\n    assert_eq!(file.read(&mut tail).unwrap(), 0);\n    let digest = format!(\n        "{:x}",\n        semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))\n    );\n    assert_eq!(\n        digest,\n        row["sha256"].as_str().unwrap(),\n        "actual current source drift: {}",\n        path.display()\n    );\n    String::from_utf8(bytes).unwrap()\n}'
 assert actual.count(current)==1, 'unique complete helper acquisition function'
 restored=actual.replace(current,original,1)
 assert len(restored)==6385 and sha(restored)=='d51a124525e3d41469c1db223a0b4526faa3faff1c757b9526ca0b2233358e8c', 'entire original protected helper'
 return restored

def _inventory_model_epoch_before(actual):
 # The current model requests its current manifest; historical inventories still
 # request the exact earlier manifest. Authenticate both complete helper epochs.
 if len(actual)==11107 and sha(actual)=='8aaa95e240657f3cc5cd97fc7034a8eadf36af8239cd5aaa8073520007653f54':
  return actual
 assert len(actual)==11317 and sha(actual)=='59fca51f35bc95418f9091c90237e7dd42453b0eb8ee346f3db5d953e0d03841', 'complete model-aware protected helper'
 restored=actual
 for current,before in ((b'        if row["sha256"] != "206f6e4ac63533b6f1e136ffe44c3d80f580a1009cc05688c771ab4ea4e5e99b" {\n        assert_eq!(row["sha256"], "4a04317123051b85eecd20f4840e760fa9494c4d3e59d5146669cd5916348a67");\n        let mut manifest = String::from_utf8(bytes).unwrap();', b'        let mut manifest = String::from_utf8(bytes).unwrap();'), (b'        bytes = manifest.into_bytes();\n        assert_eq!(bytes.len(), length);\n        }\n        assert_eq!(std::fs::read', b'        bytes = manifest.into_bytes();\n        assert_eq!(bytes.len(), length);\n        assert_eq!(std::fs::read')):
  assert restored.count(current)==1, 'unique manifest model epoch inverse'
  restored=restored.replace(current,before,1)
 assert len(restored)==11107 and sha(restored)=='8aaa95e240657f3cc5cd97fc7034a8eadf36af8239cd5aaa8073520007653f54', 'entire preceding protected helper'
 return restored

def _interop_model_epoch_before(actual):
 # Current and historical inventory rows share one complete physical helper.
 if len(actual)==11317 and sha(actual)=='59fca51f35bc95418f9091c90237e7dd42453b0eb8ee346f3db5d953e0d03841':
  return actual
 assert len(actual)==11833 and sha(actual)=='5851468b37ae14800e0edc14c91cc3eb7bc158b85417e954ab0c32030aa5a301', 'complete interop-model protected helper'
 restored=actual
 for current,before in [(b'    let interop_path = path == Path::new("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs");\n    let interop_epoch = interop_path && row["sha256"] == "d1edee35f9a7a97e9478264a44b7e77e4b3903bcbb87bb327602053f22afa6c8";\n    if interop_path && !interop_epoch {\n        assert_eq!(length, 233195);\n        assert_eq!(row["sha256"], "0e793e87c5913d29d090949527031177cf4ed760e1051bb1f032f21580f1782c");\n    }', b'    let interop_epoch = path == Path::new("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs");'), (b'    if interop_path {\n        assert_eq!(bytes.as_slice(), include_bytes!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs").as_slice());\n    }\n    if interop_epoch {\n        let compiled = include_bytes!', b'    if interop_epoch {\n        let compiled = include_bytes!')]:
  assert restored.count(current)==1, 'unique interop model epoch inverse'
  restored=restored.replace(current,before,1)
 assert len(restored)==11317 and sha(restored)=='59fca51f35bc95418f9091c90237e7dd42453b0eb8ee346f3db5d953e0d03841', 'entire preceding model-aware helper'
 return restored

def _next_raw_registration_helper_before(actual):
 # Closed current helper epoch only; the entire preceding helper is unchanged.
 assert len(actual)==12873 and sha(actual)=='4fa434fcfb95738842c6ed0ef8a90d8c81ee02414328d5f3fd365c26d83bbd59', 'complete next raw registration helper'
 restored=actual
 for at,after,before in reversed([[1036, '        assert_eq!(length, 233409);\n        assert_eq!(row["sha256"], "760872c3ea2924d4accd70e0b42b8573debaa312cfa4cd8a11cd61455b4701c5");\n', '        assert_eq!(length, 233195);\n        assert_eq!(row["sha256"], "0e793e87c5913d29d090949527031177cf4ed760e1051bb1f032f21580f1782c");\n'], [1361, '        233409\n', '        233195\n'], [2126, '        // Complete two-child raw epoch precedes every unchanged historical leg.\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "760872c3ea2924d4accd70e0b42b8573debaa312cfa4cd8a11cd61455b4701c5");\n        let mut next_epoch = String::from_utf8(bytes).unwrap();\n        assert_eq!(next_epoch.matches(",include_str!(\\"../../../../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\")").count(), 1);\n        next_epoch = next_epoch.replacen(",include_str!(\\"../../../../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\\")", "", 1);\n        assert_eq!(next_epoch.matches(",include_str!(\\"../../../../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\")").count(), 1);\n        next_epoch = next_epoch.replacen(",include_str!(\\"../../../../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\\")", "", 1);\n        bytes = next_epoch.into_bytes();\n        assert_eq!(bytes.len(), 233195);\n', '']]):
  after=after.encode();assert restored[at:at+len(after)]==after, 'exact helper epoch byte inverse'
  restored=restored[:at]+before.encode()+restored[at+len(after):]
 assert len(restored)==11833 and sha(restored)=='5851468b37ae14800e0edc14c91cc3eb7bc158b85417e954ab0c32030aa5a301', 'entire preceding helper'
 return restored

def _protected_helper(repo,j):
 path='tests/support/production_source_inventory_original.rs'
 assert j['helper_original']==path and j['helper_before_sha256']=='d51a124525e3d41469c1db223a0b4526faa3faff1c757b9526ca0b2233358e8c'
 actual=(repo/path).read_bytes()
 epoch=_inventory_model_epoch_before(_interop_model_epoch_before(_next_raw_registration_helper_before(_constructor903_raw_helper_before(_native966_helper_before(_native967_helper_before(_native977_helper_before(_native990_helper_before(_native_registered_helper_before(_native_v18_helper_before(_native_moved_helper_before(actual)))))))))))
 assert epoch==git(repo,'424e9611a87e66f43a82a7ce7c9b4bb56a4ec64b',path), 'complete helper source epoch'
 restored=_restore_protected_helper(epoch)
 assert (repo/path).read_bytes()==actual, 'protected helper source drift'
 return restored


def _constructor903_raw_helper_before(actual):
 # Exact new physical helper acquisition before every unchanged historical leg.
 assert len(actual)==16626 and sha(actual)=='855ab2165b03db87f3295f8008a8d6bdf3e055edf455d5f201a48fadd466baec', 'complete constructor903 raw helper'
 restored=actual
 for at,after,before in reversed([[996, '    let interop_prior_current = interop_path && row["sha256"] == "760872c3ea2924d4accd70e0b42b8573debaa312cfa4cd8a11cd61455b4701c5";\n    if interop_path && !interop_epoch && !interop_prior_current {\n        assert_eq!(length, 235724);\n        assert_eq!(row["sha256"], "23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5");\n', '    if interop_path && !interop_epoch {\n        assert_eq!(length, 233409);\n        assert_eq!(row["sha256"], "760872c3ea2924d4accd70e0b42b8573debaa312cfa4cd8a11cd61455b4701c5");\n'], [1520, '        235724\n    } else if interop_prior_current {\n        assert_eq!(length, 233409);\n        235724\n', '        233409\n'], [2139, '    let native_before = |raw: Vec<u8>| {\n        assert_eq!(raw.len(), 235724);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&raw))), "23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5");\n        let mut text = String::from_utf8(raw).unwrap();\n        for literal in [\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/retained_scratch.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/uncached_peak_before.rs\\")",\n            ",include_str!(\\"../../../../../src/loan_plan/owned_capacity.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/type_proof_tests.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/tests/builder_limits.rs\\")",\n            ",include_str!(\\"../../../../../src/bounded_output/stage_debits.rs\\")",\n            ",include_str!(\\"../../../../../src/cleanup_plan/replay/factored/oracle_order/tests.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/diagnostics.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\\")",\n            ",include_str!(\\"../../../../../src/workspace_graph/diagnostics/borrowed_help_tests.rs\\")",\n        ] {\n            assert_eq!(text.matches(literal).count(), 1);\n            text = text.replacen(literal, "", 1);\n        }\n        assert_eq!(text.len(), 233409);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))), "760872c3ea2924d4accd70e0b42b8573debaa312cfa4cd8a11cd61455b4701c5");\n        text.into_bytes()\n    };\n    if interop_prior_current { bytes = native_before(bytes); }\n', ''], [5841, '        bytes = native_before(bytes);\n', '']]):
  after=after.encode();assert restored[at:at+len(after)]==after, 'exact constructor903 helper byte inverse'
  restored=restored[:at]+before.encode()+restored[at+len(after):]
 assert len(restored)==12873 and sha(restored)=='4fa434fcfb95738842c6ed0ef8a90d8c81ee02414328d5f3fd365c26d83bbd59', 'entire preceding raw helper'
 return restored


def _native966_helper_before(actual):
 assert len(actual)==17612 and sha(actual)=='53a5b40b13dcb20fa2807750cc5f91caa3ca388bd1a4a607119103d14b8580fe', 'complete native966 helper'
 parts=[];cursor=0
 for e in [{'start': 1129, 'end': 1397, 'after': '    let interop_current = interop_path && row["sha256"] == "22c4e696c83d5e53b8b2fdb5e0f5b00a7e6b40e9bb53fffbc0fc4e005152b257";\n    if interop_current { assert_eq!(length, 239547); }\n    if interop_path && !interop_epoch && !interop_prior_current && !interop_current {\n', 'before': '    if interop_path && !interop_epoch && !interop_prior_current {\n'}, {'start': 1849, 'end': 1920, 'after': '    let read_length = if interop_path { 239547 } else { read_length };\n', 'before': ''}, {'start': 2406, 'end': 3037, 'after': '    }\n    if interop_path {\n        assert_eq!(bytes.len(), 239547);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "22c4e696c83d5e53b8b2fdb5e0f5b00a7e6b40e9bb53fffbc0fc4e005152b257");\n        if !interop_current {\n            let prior = include_bytes!("../../e/23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5");\n            assert_eq!(prior.len(), 235724);\n            assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(prior))), "23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5");\n            bytes = prior.to_vec();\n        }\n', 'before': ''}, {'start': 6686, 'end': 6827, 'after': '        assert_eq!(bytes.as_slice(), include_bytes!("../../e/23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5").as_slice());\n', 'before': '        assert_eq!(bytes.as_slice(), compiled.as_slice());\n'}]:
  assert actual[e['start']:e['end']]==e['after'].encode(), 'exact native966 helper inverse'
  parts.extend((actual[cursor:e['start']],e['before'].encode()));cursor=e['end']
 restored=b''.join(parts+[actual[cursor:]])
 assert len(restored)==16626 and sha(restored)=='855ab2165b03db87f3295f8008a8d6bdf3e055edf455d5f201a48fadd466baec', 'whole preceding constructor903 helper'
 return restored


def _native967_helper_before(actual):
 assert len(actual)==18450 and sha(actual)=='47dacdb96f8d92e0cea93a1126ce21511641baf3be5cbce3883b03cb1bf1f607', 'complete native967 helper'
 parts=[];cursor=0
 for e in [{'start': 871, 'end': 1047, 'after': '    let interop_next = interop_path && row["sha256"] == "a056e62b5dc75528b2d19f5469b08ab831fbf48c2f8348154a3e360c341c5b4b";\n    if interop_next { assert_eq!(length, 239622); }\n', 'before': ''}, {'start': 1487, 'end': 1590, 'after': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next {\n', 'before': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current {\n'}, {'start': 2042, 'end': 2113, 'after': '    let read_length = if interop_path { 239622 } else { read_length };\n', 'before': '    let read_length = if interop_path { 239547 } else { read_length };\n'}, {'start': 2627, 'end': 3272, 'after': '        assert_eq!(bytes.len(), 239622);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "a056e62b5dc75528b2d19f5469b08ab831fbf48c2f8348154a3e360c341c5b4b");\n        if !interop_next {\n            let prior = include_bytes!("../../e/22c4e696c83d5e53b8b2fdb5e0f5b00a7e6b40e9bb53fffbc0fc4e005152b257");\n            assert_eq!(prior.len(), 239547);\n            assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(prior))), "22c4e696c83d5e53b8b2fdb5e0f5b00a7e6b40e9bb53fffbc0fc4e005152b257");\n            bytes = prior.to_vec();\n        }\n    }\n    if interop_path && !interop_next {\n', 'before': ''}]:
  assert actual[e['start']:e['end']]==e['after'].encode(), 'exact native967 helper inverse'
  parts.extend((actual[cursor:e['start']],e['before'].encode()));cursor=e['end']
 restored=b''.join(parts+[actual[cursor:]])
 assert len(restored)==17612 and sha(restored)=='53a5b40b13dcb20fa2807750cc5f91caa3ca388bd1a4a607119103d14b8580fe', 'whole preceding native966 helper'
 return restored


def _native977_helper_before(actual):
 assert len(actual)==19323 and sha(actual)=='181b90aba2cfbabb1d7f0092aa212f62042aec09eaf18a82f6182bf69bbbbd48', 'complete next977 native helper'
 cursor=0;parts=[]
 for e in [{'start': 871, 'end': 1053, 'after': '    let interop_next977 = interop_path && row["sha256"] == "677cc0d000dfc7a179d0dc07956f9b3f9ca68c4ee2f14e1d134e1c474004f1b6";\n    if interop_next977 { assert_eq!(length, 240728); }\n', 'before': ''}, {'start': 1669, 'end': 1792, 'after': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 {\n', 'before': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next {\n'}, {'start': 2244, 'end': 2315, 'after': '    let read_length = if interop_path { 240728 } else { read_length };\n', 'before': '    let read_length = if interop_path { 239622 } else { read_length };\n'}, {'start': 2829, 'end': 3480, 'after': '        assert_eq!(bytes.len(), 240728);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "677cc0d000dfc7a179d0dc07956f9b3f9ca68c4ee2f14e1d134e1c474004f1b6");\n        if !interop_next977 {\n            let prior = include_bytes!("../../e/a056e62b5dc75528b2d19f5469b08ab831fbf48c2f8348154a3e360c341c5b4b");\n            assert_eq!(prior.len(), 239622);\n            assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(prior))), "a056e62b5dc75528b2d19f5469b08ab831fbf48c2f8348154a3e360c341c5b4b");\n            bytes = prior.to_vec();\n        }\n    }\n    if interop_path && !interop_next977 {\n', 'before': ''}, {'start': 4086, 'end': 4145, 'after': '    if interop_path && !interop_next && !interop_next977 {\n', 'before': '    if interop_path && !interop_next {\n'}]:
  assert actual[e['start']:e['end']]==e['after'].encode(), 'exact next977 whole helper inverse'
  parts.extend((actual[cursor:e['start']],e['before'].encode()));cursor=e['end']
 restored=b''.join(parts+[actual[cursor:]])
 assert len(restored)==18450 and sha(restored)=='47dacdb96f8d92e0cea93a1126ce21511641baf3be5cbce3883b03cb1bf1f607', 'whole preceding native967 helper'
 return restored


def _native990_helper_before(actual):
 assert len(actual)==22115 and sha(actual)=='c6f229dad25e8a318f56441588946c3da47444e41259a1278f9cfdf91be77b95', 'complete native990 helper'
 cursor=0;parts=[]
 for e in [{'start': 871, 'end': 1053, 'after': '    let interop_next990 = interop_path && row["sha256"] == "e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98";\n    if interop_next990 { assert_eq!(length, 241368); }\n', 'before': ''}, {'start': 1851, 'end': 1994, 'after': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 && !interop_next990 {\n', 'before': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 {\n'}, {'start': 2446, 'end': 2517, 'after': '    let read_length = if interop_path { 241368 } else { read_length };\n', 'before': '    let read_length = if interop_path { 240728 } else { read_length };\n'}, {'start': 3009, 'end': 3666, 'after': '    if interop_path {\n        assert_eq!(bytes.len(), 241368);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98");\n        if !interop_next990 {\n            let prior = include_bytes!("../../e/677cc0d000dfc7a179d0dc07956f9b3f9ca68c4ee2f14e1d134e1c474004f1b6");\n            assert_eq!(prior.len(), 240728);\n            assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(prior))), "677cc0d000dfc7a179d0dc07956f9b3f9ca68c4ee2f14e1d134e1c474004f1b6");\n            bytes = prior.to_vec();\n        }\n    }\n    if !interop_next990 {\n', 'before': ''}, {'start': 13122, 'end': 13128, 'after': '    }\n', 'before': ''}, {'start': 15052, 'end': 16979, 'after': '    if interop_path {\n        let physical_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);\n        let mut options = std::fs::OpenOptions::new();\n        options.read(true);\n        #[cfg(unix)] {\n            use std::os::unix::fs::OpenOptionsExt;\n            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);\n        }\n        #[cfg(windows)] {\n            use std::os::windows::fs::OpenOptionsExt;\n            options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT: inspect, never follow.\n        }\n        #[cfg(not(any(unix, windows)))]\n        panic!("native990 custody requires a nofollow regular-file reader");\n        let identity = |m: std::fs::Metadata| {\n            assert!(m.is_file() && !m.file_type().is_symlink());\n            #[cfg(unix)] {\n                use std::os::unix::fs::MetadataExt;\n                (m.dev(), m.ino(), m.mode(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec())\n            }\n            #[cfg(not(unix))] { (m.len(), m.created().unwrap(), m.modified().unwrap()) }\n        };\n        let file = options.open(&physical_path).unwrap();\n        let metadata = file.metadata().unwrap();\n        assert_eq!(metadata.len(), 241368);\n        let before_identity = identity(metadata);\n        assert_eq!(before_identity, identity(std::fs::symlink_metadata(&physical_path).unwrap()));\n        let mut later = Vec::new();\n        later.try_reserve_exact(241369).unwrap();\n        (&file).take(241369).read_to_end(&mut later).unwrap();\n        assert_eq!(later.len(), 241368);\n        assert_eq!(later.as_slice(), include_bytes!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs").as_slice());\n        assert_eq!(before_identity, identity(file.metadata().unwrap()));\n        assert_eq!(before_identity, identity(std::fs::symlink_metadata(&physical_path).unwrap()));\n    }\n', 'before': ''}]:
  assert actual[e['start']:e['end']]==e['after'].encode(), 'exact native990 whole helper inverse'
  parts.extend((actual[cursor:e['start']],e['before'].encode()));cursor=e['end']
 restored=b''.join(parts+[actual[cursor:]])
 assert len(restored)==19323 and sha(restored)=='181b90aba2cfbabb1d7f0092aa212f62042aec09eaf18a82f6182bf69bbbbd48', 'whole preceding native977 helper'
 return restored


def _native_registered_helper_before(actual):
 assert len(actual)==24682 and sha(actual)=='e1b1fa575a68d6c503e21dd2c94ad24e03ac21060d4293396bcad043adb022fa', 'complete registered RAW helper'
 cursor=0;parts=[]
 for e in [{'start': 871, 'end': 1059, 'after': '    let interop_registered = interop_path && row["sha256"] == "c62b6af0b063d006f709d5eec28c804f367fbee9f96319861c9d4b50394281c3";\n    if interop_registered { assert_eq!(length, 242643); }\n', 'before': ''}, {'start': 2039, 'end': 2205, 'after': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 && !interop_next990 && !interop_registered {\n', 'before': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 && !interop_next990 {\n'}, {'start': 2657, 'end': 2728, 'after': '    let read_length = if interop_path { 242643 } else { read_length };\n', 'before': '    let read_length = if interop_path { 241368 } else { read_length };\n'}, {'start': 3220, 'end': 3554, 'after': '    if interop_path {\n        assert_eq!(bytes.len(), 242643);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "c62b6af0b063d006f709d5eec28c804f367fbee9f96319861c9d4b50394281c3");\n        if !interop_registered { bytes = native_before_registered_raw(); }\n    }\n    if !interop_registered {\n', 'before': ''}, {'start': 13673, 'end': 13679, 'after': '    }\n', 'before': ''}, {'start': 16790, 'end': 16834, 'after': '        assert_eq!(metadata.len(), 242643);\n', 'before': '        assert_eq!(metadata.len(), 241368);\n'}, {'start': 17019, 'end': 17173, 'after': '        later.try_reserve_exact(242644).unwrap();\n        (&file).take(242644).read_to_end(&mut later).unwrap();\n        assert_eq!(later.len(), 242643);\n', 'before': '        later.try_reserve_exact(241369).unwrap();\n        (&file).take(241369).read_to_end(&mut later).unwrap();\n        assert_eq!(later.len(), 241368);\n'}, {'start': 22666, 'end': 24682, 'after': '\n// Exact immediate predecessor archive; no generic source alias or discovery.\nfn native_before_registered_raw() -> Vec<u8> {\n    let compiled = include_bytes!("../../e/e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98");\n    assert_eq!(compiled.len(), 241368);\n    assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(compiled))), "e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98");\n    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("e/e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98");\n    let mut options = std::fs::OpenOptions::new();\n    options.read(true);\n    #[cfg(unix)] {\n        use std::os::unix::fs::OpenOptionsExt;\n        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);\n    }\n    #[cfg(windows)] {\n        use std::os::windows::fs::OpenOptionsExt;\n        options.custom_flags(0x0020_0000);\n    }\n    #[cfg(not(any(unix, windows)))]\n    panic!("registered RAW custody requires a nofollow regular-file reader");\n    let identity = |m: std::fs::Metadata| {\n        assert!(m.is_file() && !m.file_type().is_symlink());\n        #[cfg(unix)] {\n            use std::os::unix::fs::MetadataExt;\n            (m.dev(), m.ino(), m.mode(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec())\n        }\n        #[cfg(not(unix))] { (m.len(), m.created().unwrap(), m.modified().unwrap()) }\n    };\n    let file = options.open(&path).unwrap();\n    let before = identity(file.metadata().unwrap());\n    assert_eq!(before, identity(std::fs::symlink_metadata(&path).unwrap()));\n    assert_eq!(file.metadata().unwrap().len(), 241368);\n    assert!(241369 <= 2 * 1024 * 1024);\n    let mut raw = Vec::new();\n    raw.try_reserve_exact(241369).unwrap();\n    (&file).take(241369).read_to_end(&mut raw).unwrap();\n    assert_eq!(raw.as_slice(), compiled.as_slice());\n    assert_eq!(before, identity(file.metadata().unwrap()));\n    assert_eq!(before, identity(std::fs::symlink_metadata(&path).unwrap()));\n    raw\n}\n', 'before': ''}]:
  assert actual[e['start']:e['end']]==e['after'].encode(), 'exact registered RAW helper inverse'
  parts.extend((actual[cursor:e['start']],e['before'].encode()));cursor=e['end']
 restored=b''.join(parts+[actual[cursor:]])
 assert len(restored)==22115 and sha(restored)=='c6f229dad25e8a318f56441588946c3da47444e41259a1278f9cfdf91be77b95', 'ENTIRE immediate native990 helper'
 return restored


def _native_v18_helper_before(actual):
 assert len(actual)==27239 and sha(actual)=='272c33756db75ad7c522bcd7756fd128843403e3b3f677c14e1935dd1a31551e', 'complete V18 RAW helper'
 cursor=0;parts=[]
 for e in [{'start': 871, 'end': 1059, 'after': '    let interop_typed_refs = interop_path && row["sha256"] == "17d537133c8cd539c13f16e86794187aba605db08564507eb8548edf671a9f79";\n    if interop_typed_refs { assert_eq!(length, 242747); }\n', 'before': ''}, {'start': 2227, 'end': 2416, 'after': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 && !interop_next990 && !interop_registered && !interop_typed_refs {\n', 'before': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 && !interop_next990 && !interop_registered {\n'}, {'start': 2868, 'end': 2939, 'after': '    let read_length = if interop_path { 242747 } else { read_length };\n', 'before': '    let read_length = if interop_path { 242643 } else { read_length };\n'}, {'start': 3431, 'end': 3765, 'after': '    if interop_path {\n        assert_eq!(bytes.len(), 242747);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "17d537133c8cd539c13f16e86794187aba605db08564507eb8548edf671a9f79");\n        if !interop_typed_refs { bytes = native_before_typed_refs_raw(); }\n    }\n    if !interop_typed_refs {\n', 'before': ''}, {'start': 14224, 'end': 14230, 'after': '    }\n', 'before': ''}, {'start': 17341, 'end': 17385, 'after': '        assert_eq!(metadata.len(), 242747);\n', 'before': '        assert_eq!(metadata.len(), 242643);\n'}, {'start': 17570, 'end': 17724, 'after': '        later.try_reserve_exact(242748).unwrap();\n        (&file).take(242748).read_to_end(&mut later).unwrap();\n        assert_eq!(later.len(), 242747);\n', 'before': '        later.try_reserve_exact(242644).unwrap();\n        (&file).take(242644).read_to_end(&mut later).unwrap();\n        assert_eq!(later.len(), 242643);\n'}, {'start': 25233, 'end': 27239, 'after': '\n// Exact V18 registration predecessor, independently authenticated.\nfn native_before_typed_refs_raw() -> Vec<u8> {\n    let compiled = include_bytes!("../../e/c62b6af0b063d006f709d5eec28c804f367fbee9f96319861c9d4b50394281c3");\n    assert_eq!(compiled.len(), 242643);\n    assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(compiled))), "c62b6af0b063d006f709d5eec28c804f367fbee9f96319861c9d4b50394281c3");\n    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("e/c62b6af0b063d006f709d5eec28c804f367fbee9f96319861c9d4b50394281c3");\n    let mut options = std::fs::OpenOptions::new();\n    options.read(true);\n    #[cfg(unix)] {\n        use std::os::unix::fs::OpenOptionsExt;\n        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);\n    }\n    #[cfg(windows)] {\n        use std::os::windows::fs::OpenOptionsExt;\n        options.custom_flags(0x0020_0000);\n    }\n    #[cfg(not(any(unix, windows)))]\n    panic!("registered RAW custody requires a nofollow regular-file reader");\n    let identity = |m: std::fs::Metadata| {\n        assert!(m.is_file() && !m.file_type().is_symlink());\n        #[cfg(unix)] {\n            use std::os::unix::fs::MetadataExt;\n            (m.dev(), m.ino(), m.mode(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec())\n        }\n        #[cfg(not(unix))] { (m.len(), m.created().unwrap(), m.modified().unwrap()) }\n    };\n    let file = options.open(&path).unwrap();\n    let before = identity(file.metadata().unwrap());\n    assert_eq!(before, identity(std::fs::symlink_metadata(&path).unwrap()));\n    assert_eq!(file.metadata().unwrap().len(), 242643);\n    assert!(242644 <= 2 * 1024 * 1024);\n    let mut raw = Vec::new();\n    raw.try_reserve_exact(242644).unwrap();\n    (&file).take(242644).read_to_end(&mut raw).unwrap();\n    assert_eq!(raw.as_slice(), compiled.as_slice());\n    assert_eq!(before, identity(file.metadata().unwrap()));\n    assert_eq!(before, identity(std::fs::symlink_metadata(&path).unwrap()));\n    raw\n}\n', 'before': ''}]:
  assert actual[e['start']:e['end']]==e['after'].encode(), 'exact V18 RAW helper inverse'
  parts.extend((actual[cursor:e['start']],e['before'].encode()));cursor=e['end']
 restored=b''.join(parts+[actual[cursor:]])
 assert len(restored)==24682 and sha(restored)=='e1b1fa575a68d6c503e21dd2c94ad24e03ac21060d4293396bcad043adb022fa', 'ENTIRE immediate registered RAW helper'
 return restored


def _native_moved_helper_before(actual):
 assert len(actual)==29819 and sha(actual)=='c97d4c6ef74218d77aae7d8eb2424b455fbef1ff26ee01d53a49e3e7f3c1e3c1', 'complete moved-direct RAW helper'
 cursor=0;parts=[]
 for e in [{'start': 871, 'end': 1063, 'after': '    let interop_moved_direct = interop_path && row["sha256"] == "f0403b893a95302b76a41d0dc33709fc233ad6f5b479e819ec10be6dff9efaf9";\n    if interop_moved_direct { assert_eq!(length, 243027); }\n', 'before': ''}, {'start': 2419, 'end': 2633, 'after': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 && !interop_next990 && !interop_registered && !interop_typed_refs && !interop_moved_direct {\n', 'before': '    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 && !interop_next990 && !interop_registered && !interop_typed_refs {\n'}, {'start': 3085, 'end': 3156, 'after': '    let read_length = if interop_path { 243027 } else { read_length };\n', 'before': '    let read_length = if interop_path { 242747 } else { read_length };\n'}, {'start': 3648, 'end': 3988, 'after': '    if interop_path {\n        assert_eq!(bytes.len(), 243027);\n        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "f0403b893a95302b76a41d0dc33709fc233ad6f5b479e819ec10be6dff9efaf9");\n        if !interop_moved_direct { bytes = native_before_moved_direct_raw(); }\n    }\n    if !interop_moved_direct {\n', 'before': ''}, {'start': 14787, 'end': 14793, 'after': '    }\n', 'before': ''}, {'start': 17904, 'end': 17948, 'after': '        assert_eq!(metadata.len(), 243027);\n', 'before': '        assert_eq!(metadata.len(), 242747);\n'}, {'start': 18133, 'end': 18287, 'after': '        later.try_reserve_exact(243028).unwrap();\n        (&file).take(243028).read_to_end(&mut later).unwrap();\n        assert_eq!(later.len(), 243027);\n', 'before': '        later.try_reserve_exact(242748).unwrap();\n        (&file).take(242748).read_to_end(&mut later).unwrap();\n        assert_eq!(later.len(), 242747);\n'}, {'start': 27802, 'end': 29819, 'after': '\n// Exact moved-direct registration predecessor, independently authenticated.\nfn native_before_moved_direct_raw() -> Vec<u8> {\n    let compiled = include_bytes!("../../e/17d537133c8cd539c13f16e86794187aba605db08564507eb8548edf671a9f79");\n    assert_eq!(compiled.len(), 242747);\n    assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(compiled))), "17d537133c8cd539c13f16e86794187aba605db08564507eb8548edf671a9f79");\n    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("e/17d537133c8cd539c13f16e86794187aba605db08564507eb8548edf671a9f79");\n    let mut options = std::fs::OpenOptions::new();\n    options.read(true);\n    #[cfg(unix)] {\n        use std::os::unix::fs::OpenOptionsExt;\n        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);\n    }\n    #[cfg(windows)] {\n        use std::os::windows::fs::OpenOptionsExt;\n        options.custom_flags(0x0020_0000);\n    }\n    #[cfg(not(any(unix, windows)))]\n    panic!("registered RAW custody requires a nofollow regular-file reader");\n    let identity = |m: std::fs::Metadata| {\n        assert!(m.is_file() && !m.file_type().is_symlink());\n        #[cfg(unix)] {\n            use std::os::unix::fs::MetadataExt;\n            (m.dev(), m.ino(), m.mode(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec())\n        }\n        #[cfg(not(unix))] { (m.len(), m.created().unwrap(), m.modified().unwrap()) }\n    };\n    let file = options.open(&path).unwrap();\n    let before = identity(file.metadata().unwrap());\n    assert_eq!(before, identity(std::fs::symlink_metadata(&path).unwrap()));\n    assert_eq!(file.metadata().unwrap().len(), 242747);\n    assert!(242748 <= 2 * 1024 * 1024);\n    let mut raw = Vec::new();\n    raw.try_reserve_exact(242748).unwrap();\n    (&file).take(242748).read_to_end(&mut raw).unwrap();\n    assert_eq!(raw.as_slice(), compiled.as_slice());\n    assert_eq!(before, identity(file.metadata().unwrap()));\n    assert_eq!(before, identity(std::fs::symlink_metadata(&path).unwrap()));\n    raw\n}\n', 'before': ''}]:
  assert actual[e['start']:e['end']]==e['after'].encode(), 'exact moved-direct RAW helper inverse'
  parts.extend((actual[cursor:e['start']],e['before'].encode()));cursor=e['end']
 restored=b''.join(parts+[actual[cursor:]])
 assert len(restored)==27239 and sha(restored)=='272c33756db75ad7c522bcd7756fd128843403e3b3f677c14e1935dd1a31551e', 'ENTIRE immediate V18 RAW helper'
 return restored
