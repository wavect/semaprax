"""Run the complete original FINAL checker with optional pure derived reuse."""
from pathlib import Path
import hashlib,sys,os,stat
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
def bounded(path,length,digest):
 assert 0<=length<=262144 and hasattr(os,'O_NOFOLLOW')
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 identity=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode) and first.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as stream:
   raw=stream.read(length);assert len(raw)==length and not stream.read(1)
  after=os.fstat(fd);physical=os.lstat(path)
  assert stat.S_ISREG(physical.st_mode) and identity(first)==identity(after)==identity(physical)
  assert hashlib.sha256(raw).hexdigest()==digest
  raw.decode('utf8');return raw
 finally:os.close(fd)
_INCOMING48_LIVE_EDITS = [{'start': 11696, 'end': 11699, 'after': '609', 'before': ''}, {'start': 11700, 'end': 11700, 'after': '', 'before': '427'}, {'start': 11703, 'end': 11703, 'after': '', 'before': 'cf84acd'}, {'start': 11704, 'end': 11706, 'after': '11', 'before': 'cb'}, {'start': 11707, 'end': 11707, 'after': '', 'before': 'd9'}, {'start': 11708, 'end': 11708, 'after': '', 'before': 'f'}, {'start': 11709, 'end': 11709, 'after': '', 'before': '4fa5b'}, {'start': 11711, 'end': 11743, 'after': '270644548b1c99eb29628c90a29c5ca6', 'before': ''}, {'start': 11746, 'end': 11746, 'after': '', 'before': '3185d7f'}, {'start': 11749, 'end': 11753, 'after': '6827', 'before': 'b3d'}, {'start': 11755, 'end': 11756, 'after': '9', 'before': 'dd83a333edf6458b0c5e'}, {'start': 11757, 'end': 11758, 'after': 'c', 'before': ''}, {'start': 11759, 'end': 11766, 'after': '2bea20f', 'before': ''}, {'start': 11810, 'end': 11813, 'after': '740', 'before': '512'}, {'start': 11816, 'end': 11816, 'after': '', 'before': '63e15c9f08db2abe227073c6fc5a3440e2'}, {'start': 11818, 'end': 11818, 'after': '', 'before': 'e8d3cfb0daf42adb'}, {'start': 11819, 'end': 11823, 'after': 'ddd0', 'before': 'fbe'}, {'start': 11824, 'end': 11824, 'after': '', 'before': '38'}, {'start': 11825, 'end': 11832, 'after': '7668881', 'before': ''}, {'start': 11834, 'end': 11834, 'after': '', 'before': '2'}, {'start': 11835, 'end': 11880, 'after': '0a2a583df6821fd8ee8ad9b469300ae824dcb4958fe3a', 'before': ''}, {'start': 12054, 'end': 12054, 'after': '', 'before': '2329, "cdb8de136965391db8902aa21a61c2b'}, {'start': 12057, 'end': 12057, 'after': '', 'before': '1f50b6'}, {'start': 12058, 'end': 12063, 'after': ', "6c', 'before': ''}, {'start': 12064, 'end': 12067, 'after': '977', 'before': 'd5'}, {'start': 12068, 'end': 12071, 'after': '38a', 'before': 'd'}, {'start': 12072, 'end': 12073, 'after': 'a', 'before': ''}, {'start': 12074, 'end': 12083, 'after': 'd364b2adc', 'before': ''}, {'start': 12085, 'end': 12101, 'after': '4aaf648b322f4b6f', 'before': '7'}, {'start': 12102, 'end': 12119, 'after': '6e44978c44a4b4190', 'before': ''}, {'start': 12121, 'end': 12123, 'after': 'e7', 'before': '925833'}, {'start': 12124, 'end': 12125, 'after': '9', 'before': '3d1'}, {'start': 12190, 'end': 12191, 'after': '8', 'before': '5'}, {'start': 12192, 'end': 12194, 'after': '88', 'before': '24'}, {'start': 12197, 'end': 12197, 'after': '', 'before': '9'}, {'start': 12198, 'end': 12200, 'after': '8f', 'before': 'ec'}, {'start': 12202, 'end': 12206, 'after': '48dc', 'before': ''}, {'start': 12208, 'end': 12224, 'after': 'b8ef5483f7d8ccf9', 'before': ''}, {'start': 12227, 'end': 12230, 'after': 'cd4', 'before': '235'}, {'start': 12231, 'end': 12233, 'after': 'e0', 'before': ''}, {'start': 12234, 'end': 12240, 'after': '3cd44e', 'before': ''}, {'start': 12241, 'end': 12250, 'after': '0c8963362', 'before': 'f'}, {'start': 12253, 'end': 12255, 'after': '00', 'before': '7b'}, {'start': 12256, 'end': 12256, 'after': '', 'before': '9d314918b4547a3a1fbac6'}, {'start': 12258, 'end': 12259, 'after': '6', 'before': '3c8e487'}, {'start': 12260, 'end': 12261, 'after': '3', 'before': '0f579757'}, {'start': 12380, 'end': 12380, 'after': '', 'before': '1'}, {'start': 12381, 'end': 12384, 'after': '333', 'before': '68'}, {'start': 12387, 'end': 12387, 'after': '', 'before': 'e3'}, {'start': 12388, 'end': 12388, 'after': '', 'before': 'f6b60'}, {'start': 12389, 'end': 12389, 'after': '', 'before': '614a9a7e902a7'}, {'start': 12392, 'end': 12398, 'after': 'a5b87c', 'before': '6d9'}, {'start': 12401, 'end': 12401, 'after': '', 'before': '752a5d68d0'}, {'start': 12402, 'end': 12405, 'after': 'fd3', 'before': '8986'}, {'start': 12407, 'end': 12414, 'after': '7d15e75', 'before': '28a4'}, {'start': 12415, 'end': 12415, 'after': '', 'before': '6'}, {'start': 12416, 'end': 12417, 'after': '1', 'before': '39a'}, {'start': 12418, 'end': 12421, 'after': 'ad5', 'before': ''}, {'start': 12422, 'end': 12423, 'after': '0', 'before': '6'}, {'start': 12425, 'end': 12434, 'after': '6c43a799d', 'before': ''}, {'start': 12435, 'end': 12451, 'after': '6aa1cb99b27a911f', 'before': ''}, {'start': 18745, 'end': 23170, 'after': '// Entire incoming f65 successor source views are compiled historical archives.\n// They never replace the separately authenticated current physical owners.\nconst INCOMING_SUCCESSOR_VIEWS: [(&str, &str); 6] = [\n    (\n        "src/cleanup_plan/replay.rs",\n        include_str!("../../e/bdce82425869a91e406c78175753f19ff0db46a468c4bba33f8bef793b4fe234"),\n    ),\n    (\n        "src/source_reflows_merge.rs",\n        include_str!("../../e/5e5db20c191765da259132014f7945f5b651208102778a2381821f31d0805229"),\n    ),\n    (\n        "src/source_reflows_merge/main67107_inputs.rs",\n        include_str!("../../e/555d676283e593101b9fc21b8e26f71b1b31b9567c52d3ca402077c4f8bf7fab"),\n    ),\n    (\n        "tests/hir_module_boundaries.rs",\n        include_str!("../../e/246d91d06278340a16c3ef24d6660a292f68f74ff4533e87ba349c062956f4f4"),\n    ),\n    (\n        "tests/workspace/semantic_graph/source_inventory.rs",\n        include_str!("../../e/0144e54abac27fcfc032f9bc873fa616539da1a2b20de17495b84f68428104e7"),\n    ),\n    (\n        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",\n        include_str!("../../e/3a43d350fbe971a7c3bb46000b559fb3c1bc5995e196953382068149ee82c96f"),\n    ),\n];\nfn historical_successor_ingress_checkpoint(\n    path: &str,\n    compiled: &str,\n    previous: &serde_json::Value,\n) -> (String, String) {\n    const PATH: &str = "docs/evidence/ordinary-ingress-successor-20261026/current-inputs.json";\n    const DATA: &str = include_str!(concat!(\n        env!("CARGO_MANIFEST_DIR"),\n        "/docs/evidence/ordinary-ingress-successor-20261026/current-inputs.json"\n    ));\n    const LENGTH: usize = 14971;\n    const SHA: &str = "aef6ee23a1724873cd15d3590f0086fc8a304ce3db94ca4525989994e8360046";\n    const { assert!(LENGTH <= FINAL_INGRESS_CAP) };\n    assert_eq!(DATA.len(), LENGTH);\n    assert_eq!(digest(DATA), SHA);\n    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));\n    assert_eq!(final_ingress_file(&root.join(PATH), LENGTH, SHA), DATA);\n    let packet: serde_json::Value = serde_json::from_str(DATA).unwrap();\n    assert_eq!(\n        packet["schema"],\n        "semaprax.ordinary-plan-source-successor.v1"\n    );\n    let rows = packet["rows"].as_array().unwrap();\n    assert_eq!(rows.len(), CURRENT.len());\n    for (row, (source, _)) in rows.iter().zip(CURRENT) {\n        assert_eq!(row["path"], source, "closed ordered physical source owners");\n    }\n    let row = rows.iter().find(|row| row["path"] == path).unwrap();\n    assert_eq!(row["previous_length"], previous["current_length"]);\n    assert_eq!(row["previous_sha256"], previous["current_sha256"]);\n    assert_eq!(row["previous_archive"], previous["current_archive"]);\n    let length = usize::try_from(row["current_length"].as_u64().unwrap()).unwrap();\n    let sha = row["current_sha256"].as_str().unwrap();\n    let historical_archive = row["current_archive"].as_str().unwrap();\n    assert_eq!(historical_archive, format!("e/{sha}"));\n    let live = final_ingress_file(&root.join(historical_archive), length, sha);\n    assert_eq!(\n        live, compiled,\n        "complete incoming compiled and physical historical archive"\n    );\n    let archive = row["current_archive"].as_str().unwrap();\n    assert_eq!(archive, format!("e/{sha}"));\n    assert_eq!(final_ingress_file(&root.join(archive), length, sha), live);\n    let mut restored = live.clone();\n    let edits = row["inverse"].as_array().unwrap();\n    assert!(edits.len() <= 512);\n    let mut last = restored.len();\n    for edit in edits {\n        let start = usize::try_from(edit["start_byte"].as_u64().unwrap()).unwrap();\n        let after = edit["after"].as_str().unwrap();\n        let end = start\n            .checked_add(after.len())\n            .expect("bounded successor inverse");\n        assert!(end <= last);\n        assert_eq!(restored.get(start..end), Some(after));\n        restored.replace_range(start..end, edit["before"].as_str().unwrap());\n        last = start;\n    }\n    assert_eq!(\n        restored.len(),\n        usize::try_from(previous["current_length"].as_u64().unwrap()).unwrap()\n    );\n    assert_eq!(digest(&restored), previous["current_sha256"]);\n    assert_eq!(\n        restored,\n        final_ingress_file(\n            &root.join(previous["current_archive"].as_str().unwrap()),\n            restored.len(),\n            previous["current_sha256"].as_str().unwrap(),\n        )\n    );\n    (live, restored)\n}\n\n', 'before': ''}, {'start': 25806, 'end': 26240, 'after': ');\n        let historical_compiled = INCOMING_SUCCESSOR_VIEWS\n            .iter()\n            .find(|(name, _)| *name == path)\n            .expect("six literal incoming historical owners only")\n            .1;\n        let (incoming_successor, incoming_latest) =\n            historical_successor_ingress_checkpoint(path, historical_compiled, newest);\n        assert_eq!(incoming_latest, latest, "both entire common latest stages agree"', 'before': ''}, {'start': 29449, 'end': 29502, 'after': 'incoming_successor\n                    || current == ', 'before': ''}, {'start': 30008, 'end': 30057, 'after': '\n                || current == incoming_successor', 'before': ''}]

def _incoming48_replay(current):
 import hashlib
 assert len(current)==33206 and hashlib.sha256(current).hexdigest()=='e8d92cce1daa93a4c684c268e1aadaae19a1446e2a219f7fee9eceec19c54fb0'
 value=current;last=len(value)
 for edit in reversed(_INCOMING48_LIVE_EDITS):
  start,end=edit['start'],edit['end'];after=edit['after'].encode()
  assert type(start)is int and type(end)is int and 0<=start<=end<=last
  assert value[start:end]==after
  value=value[:start]+edit['before'].encode()+value[end:];last=start
 assert len(value)==28245 and hashlib.sha256(value).hexdigest()=='f8e49cc014614d85907f81d504914cabc478e278a8217f3a0956339a2c107b9a'
 return value

# One closed current-helper epoch precedes the ENTIRE retained e8d92 producer.
_INCOMING48_JOINT_EDIT = {'start': 11610, 'end': 12457, 'after': 'const LIVE_INGRESS: [(&str, usize, &str); 6] = [\n    ("src/cleanup_plan/replay.rs", 326143, "392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a"),\n    ("src/source_reflows_merge.rs", 102740, "426ddd09476688811850a2a583df6821fd8ee8ad9b469300ae824dcb4958fe3a"),\n    ("src/source_reflows_merge/main67107_inputs.rs", 49667, "555d676283e593101b9fc21b8e26f71b1b31b9567c52d3ca402077c4f8bf7fab"),\n    ("tests/hir_module_boundaries.rs", 164775, "d38a89d107d7b8c666be6346a7a6a28ad167c740b5ba9c7f97156cc300805169"),\n    ("tests/workspace/semantic_graph/source_inventory.rs", 259361, "5a3db1c2f1beae041e3df91b474afe27a67a9e3b1b2de9668a5f549d8a30b47e"),\n    ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", 244150, "d49f3e5888a6368cd2a0271319415fdacd18e8fc9252246bfe34d2bec14d4ae2"),\n];', 'before': 'const LIVE_INGRESS: [(&str, usize, &str); 6] = [\n    ("src/cleanup_plan/replay.rs", 326095, "5112e91b270644548b1c99eb29628c90a29c5ca6505e0e6827be92ca2bea20ff"),\n    ("src/source_reflows_merge.rs", 102740, "426ddd09476688811850a2a583df6821fd8ee8ad9b469300ae824dcb4958fe3a"),\n    ("src/source_reflows_merge/main67107_inputs.rs", 49667, "555d676283e593101b9fc21b8e26f71b1b31b9567c52d3ca402077c4f8bf7fab"),\n    ("tests/hir_module_boundaries.rs", 164042, "6c2977b38abaed364b2adc284aaf648b322f4b6fd6e44978c44a4b419055e7c9"),\n    ("tests/workspace/semantic_graph/source_inventory.rs", 258788, "48f8a48dc5eb8ef5483f7d8ccf90d8cd49e0f3cd44e50c8963362b2700103693"),\n    ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", 243333, "a118da5b87cccaefd32f7d15e75cb16ad5b0e96c43a799d86aa1cb99b27a911f"),\n];'}
def _incoming48_joint_replay(current):
 assert len(current)==33206 and hashlib.sha256(current).hexdigest()=='8124798973f2284d7985ee818e5da35f603ff3a16c44a7519e500078497b6b42'
 edit=_INCOMING48_JOINT_EDIT;start,end=edit['start'],edit['end']
 assert type(start)is int and type(end)is int and 0<=start<=end<=len(current)
 assert current[start:end]==edit['after'].encode()
 prior=current[:start]+edit['before'].encode()+current[end:]
 assert len(prior)==33206 and hashlib.sha256(prior).hexdigest()=='e8d92cce1daa93a4c684c268e1aadaae19a1446e2a219f7fee9eceec19c54fb0'
 assert prior==bounded(R/'e/e8d92cce1daa93a4c684c268e1aadaae19a1446e2a219f7fee9eceec19c54fb0',33206,'e8d92cce1daa93a4c684c268e1aadaae19a1446e2a219f7fee9eceec19c54fb0')
 # Preserve the old channel's four complete-input refusals independently;
 # the caller's four current-channel refusals exercise the new outer epoch.
 for bad in (b'',prior+b' ',prior+prior,b'foreign'):
  try:_incoming48_replay(bad)
  except AssertionError:pass
  else:raise AssertionError('foreign retained immediate helper accepted')
 return _incoming48_replay(prior)

def _incoming48_named_views(b,head):
 import copy,json,re,subprocess
 actual_read=b.read
 git=lambda *args:subprocess.check_output(['git',*args],cwd=R)
 assert git('rev-parse','HEAD').decode().strip()==head
 recipe=b.load(actual_read(b.D/'recipe.json',b.SPEC_LENGTH,b.SPEC_SHA));assert tuple(recipe['keys'])==b.KEYS
 helper_path=R/recipe['helper']
 current=actual_read(helper_path,33206,'8124798973f2284d7985ee818e5da35f603ff3a16c44a7519e500078497b6b42')
 assert current==git('show',head+':'+recipe['helper'])
 old_P=_incoming48_joint_replay(current)
 assert old_P==git('show','3f736e3f04b6327ad929a00bcd23dc6018271039:'+recipe['helper'])
 # These controls exercise the exact current-channel proof, never a relaxed scrub.
 for bad in (b'',current+b' ',current+current,b'foreign'):
  try:_incoming48_joint_replay(bad)
  except AssertionError:pass
  else:raise AssertionError('foreign current helper accepted')
 prior_packet=actual_read(b.D/'current-inputs.json',786289,'31afd14c8fa4c8792aeaec9e58fea785d9414b151cc88812a4b936e1dd3502e2')
 prior_owners=actual_read(b.D/'current-manifest.json',1851,'b0b4d32ccb88ef42fb521b2e6651b25a9191d22be32e78f7be8fad40bc81274a')
 previous=b.load(prior_packet);assert len(previous)==len(recipe['checkpoints'])==6
 rows=[];sources=[];held={helper_path:current}
 for path,prior,item in zip(b.KEYS,previous,recipe['checkpoints']):
  assert prior['path']==item['path']==path
  assert prior['expected_sha256']==item['expected_sha256']
  checkpoint=actual_read(R/item['archive'],item['length'],item['sha256'])
  assert checkpoint==git('show',b.CHECKPOINT+':'+path)
  assert (prior['checkpoint_archive'],prior['checkpoint_length'],prior['checkpoint_sha256'])==(item['archive'],item['length'],item['sha256'])
  old=actual_read(R/prior['current_archive'],prior['current_length'],prior['current_sha256'])
  assert b.restore(old,prior['inverse'])==checkpoint
  raw=actual_read(R/path);assert raw==git('show',head+':'+path)
  archive='e/'+b.sha(raw);assert actual_read(R/archive,len(raw),b.sha(raw))==raw
  row=copy.deepcopy(prior);row.update(current_length=len(raw),current_sha256=b.sha(raw),current_archive=archive,inverse=b.inverse(raw,checkpoint))
  assert b.restore(raw,row['inverse'])==checkpoint
  rows.append(row);held[R/path]=raw;held[R/archive]=raw
  sources.append({'path':path,'length':len(raw),'sha256':b.sha(raw),'archive':archive,'checkpoint_length':len(checkpoint),'checkpoint_sha256':b.sha(checkpoint)})
 packet=(json.dumps(rows,indent=2)+'\n').encode();assert len(packet)<=b.CAP
 original_template=actual_read(R/recipe['helper_template'],digest=recipe['helper_template_sha256'])
 template=b.custody_source_template(original_template.decode())
 named,n=re.subn(r'const FINAL_INGRESS_LENGTH: usize = [0-9]+;',f'const FINAL_INGRESS_LENGTH: usize = {len(packet)};',template);assert n==1
 named,n=re.subn(r'const FINAL_INGRESS_SHA256: &str = "[0-9a-f]{64}";',f'const FINAL_INGRESS_SHA256: &str = "{b.sha(packet)}";',named);assert n==1
 # The original producer/scrubber is untouched; only its two original data pins vary.
 assert b.scrub(named)==b.scrub(template) and b.pins(named)==(len(packet),b.sha(packet))
 assert len(named.splitlines())<=1500
 owners=b.load(prior_owners);assert len(owners['files'])==9
 for item in owners['files']:
  path=item['path'];physical=actual_read(R/path);assert physical==git('show',head+':'+path)
  held[R/path]=physical
  view=named.encode()if R/path==helper_path else physical
  item.update(sha256=b.sha(view),lines=len(view.splitlines()));assert item['lines']<=1500
 owner_bytes=(json.dumps(owners,indent=2)+'\n').encode();assert len(owner_bytes)<=b.CAP
 # A finite namespace of three explicitly named historical-producer inputs.
 # All six source bodies and every other original read remain physical.
 views={helper_path:named.encode(),b.D/'current-inputs.json':packet,b.D/'current-manifest.json':owner_bytes}
 originals={helper_path:current,b.D/'current-inputs.json':prior_packet,b.D/'current-manifest.json':prior_owners}
 assert len(views)==len(originals)==3
 def read_named(path,length=None,digest=None):
  if path not in views:return actual_read(path,length,digest)
  physical=originals[path];assert actual_read(path,len(physical),b.sha(physical))==physical
  value=views[path];assert len(value)<=b.CAP
  if length is not None:assert len(value)==length
  if digest is not None:assert b.sha(value)==digest
  value.decode('utf8');return value
 # Exact wrong-length/hash requests still fail; the original b.read hostiles run later.
 for path,value in views.items():
  assert read_named(path,len(value),b.sha(value))==value
  for length,digest in [(len(value)+1,None),(None,'0'*64)]:
   try:read_named(path,length,digest)
   except AssertionError:pass
   else:raise AssertionError('named-view bound accepted')
 def finish():
  assert git('rev-parse','HEAD').decode().strip()==head
  for path,raw in held.items():assert actual_read(path,len(raw),b.sha(raw))==raw
  for path,raw in originals.items():assert actual_read(path,len(raw),b.sha(raw))==raw
  assert _incoming48_joint_replay(actual_read(helper_path,33206,b.sha(current)))==old_P
 evidence={'status':'CURRENT_PHYSICAL_SOURCES_WITH_EXPLICIT_NAMED_ORIGINAL_FINAL_PRODUCER_VIEW','head':head,'repository_write':False,'Rust_qualified':False,'F6_executed':False,'current_helper':{'source':recipe['helper'],'length':len(current),'sha256':b.sha(current)},'retained_P_helper':{'head':'3f736e3f04b6327ad929a00bcd23dc6018271039','length':len(old_P),'sha256':b.sha(old_P)},'retained_immediate_helper':{'path':'e/e8d92cce1daa93a4c684c268e1aadaae19a1446e2a219f7fee9eceec19c54fb0','length':33206,'sha256':'e8d92cce1daa93a4c684c268e1aadaae19a1446e2a219f7fee9eceec19c54fb0','physical_current':False},'original_template':{'source':recipe['helper_template'],'length':len(original_template),'sha256':b.sha(original_template)},'named_inputs':{str(path.relative_to(R)):{'length':len(value),'sha256':b.sha(value),'physical':False}for path,value in views.items()},'retained_whole_prior_packet':{'length':len(prior_packet),'sha256':b.sha(prior_packet)},'retained_whole_prior_owners':{'length':len(prior_owners),'sha256':b.sha(prior_owners)},'six_actual_physical_sources':sources,'new_refusal_controls':14,'original_checker_read_scrub_and_hostile_source_bodies_unchanged':True}
 return read_named,finish,evidence

def _incoming48_with_named_helper(program):
 import argparse,importlib.util,json,re,subprocess,tempfile
 parser=argparse.ArgumentParser();parser.add_argument('--expected-head',required=True);parser.add_argument('--output',type=Path);args=parser.parse_args()
 assert re.fullmatch('[0-9a-f]{40}',args.expected_head)
 if args.output:
  assert args.output.is_absolute() and args.output.parent.resolve()==args.output.parent
  assert args.output.is_relative_to('/Volumes/Semaprax/runtime-tmp')or args.output.is_relative_to('/Volumes/Semaprax/provenance')
  assert args.output!=R and R not in args.output.parents and not args.output.exists()
 original_args=sys.argv[:];original_spec=importlib.util.spec_from_file_location;transport=[]
 binder_path=R/'docs/evidence/ordinary-plan-final-ingress-20261017/bind-current-inputs.py'
 binder_raw=bounded(binder_path,9387,'cb1a3b7568fa0490b7fabf826d3f4389d64d786ea1b62ea7aa0c242c0ea45562')
 class NamedLoader:
  def create_module(self,spec):return None
  def exec_module(self,module):
   module.__file__=str(binder_path)
   exec(compile(binder_raw,str(binder_path),'exec'),module.__dict__)
   assert bounded(binder_path,9387,'cb1a3b7568fa0490b7fabf826d3f4389d64d786ea1b62ea7aa0c242c0ea45562')==binder_raw
   read,finish,evidence=_incoming48_named_views(module,args.expected_head)
   assert not transport;transport.append((finish,evidence));module.read=read
 def closed_spec(name,location,*rest,**kwargs):
  if name=='closed_final_ingress_binder'and Path(location)==binder_path:
   assert not rest and not kwargs
   return original_spec(name,location,loader=NamedLoader())
  return original_spec(name,location,*rest,**kwargs)
 try:
  importlib.util.spec_from_file_location=closed_spec
  # The unchanged original output guard still sees its allowed provenance root.
  with tempfile.TemporaryDirectory(prefix='incoming48-named-final-',dir='/Volumes/Semaprax/provenance')as temp:
   original_output=Path(temp)/'original-result.json'
   sys.argv=[original_args[0],'--expected-head',args.expected_head,'--output',str(original_output)]
   exec(compile(program,str(D/'check-source.py'),'exec'),{'__file__':str(D/'check-source.py'),'__name__':'__main__'})
   assert len(transport)==1;finish,evidence=transport[0];finish()
   raw=bounded(original_output,original_output.stat().st_size,hashlib.sha256(original_output.read_bytes()).hexdigest())
   original=json.loads(raw);assert original['status']=='PURE_SOURCE_PASS_NOT_RUST_QUALIFIED'and original['head']==args.expected_head
   result={'status':'SOURCE_ONLY_INCOMING48_WITH_AUTHENTICATED_NAMED_HELPER_PASS','head':args.expected_head,'original_incoming_result':original,'original_result_bytes':{'length':len(raw),'sha256':hashlib.sha256(raw).hexdigest()},'transport':evidence,'repository_write':False,'Cargo':False,'runtime':'UNEXECUTED'}
   if args.output:
    original_copy=args.output.with_name(args.output.name+'.original-incoming48.json');assert not original_copy.exists()
    with original_copy.open('xb')as stream:stream.write(raw)
    with args.output.open('x')as stream:stream.write(json.dumps(result,indent=2)+'\n')
   print(json.dumps({'status':result['status'],'head':args.expected_head,'repository_write':False,'Cargo':False,'runtime':'UNEXECUTED'}))
 finally:
  importlib.util.spec_from_file_location=original_spec;sys.argv=original_args

# An explicitly named incoming-parent operator retains its entire original body.
# This route is separate from the current cached and original-checker controls.
if '--incoming48-original-checker' in sys.argv:
 sys.argv.remove('--incoming48-original-checker')
 incoming=bounded(R/'e/c9b3f6b5c4bb1cc085710b8da140183444ec1d3a9d3d39b69b5491cef57aedd2',2253,'c9b3f6b5c4bb1cc085710b8da140183444ec1d3a9d3d39b69b5491cef57aedd2')
 old_guard_binding="helper_raw=bounded(helper,11549,'f7456316bbcdf82b1487d02d19d848814a43ee6cd9ea3b2c5a7266e5a997278c')"
 current_guard_binding="helper_raw=bounded(helper,22232,'d0bc72355f03fb69f0ad46911aed41eeb9b62eb0f8c3cc714df67f69e583c4fc')"
 assert incoming.decode().count(old_guard_binding)==1
 program=incoming.decode().replace(old_guard_binding,current_guard_binding,1)
 assert program.replace(current_guard_binding,old_guard_binding,1).encode()==incoming
 _incoming48_with_named_helper(program)
 raise SystemExit(0)

original=bounded(D/'before/check-source.original.py',3542,'63e0399384c8a6ccdc56b138504e4989a59cb274c0976169c69592f2e0e4917f')
uncached='--original-checker' in sys.argv
if uncached:sys.argv.remove('--original-checker')
namespace={}
if not uncached:
 helper=bounded(D/'derived_compose.py',4597,'18ba0b7cf6ff5f2c2df9fd6f2b482336bfdf22612fca7f88fbc1a223aa54fb83')
 namespace={'__file__':str(D/'derived_compose.py')}
 exec(compile(helper,namespace['__file__'],'exec'),namespace)
program=original.decode()
# The immutable old wrapper retains its complete authenticated original bytes.
# This exact input-pin transport applies equally to cached and uncached execution.
old_binding="helper_raw=bounded(helper,19144,'af70eaeeb1cc0da02460858f9e7ddb39086582add858c74138c4d989644e11f3')"
current_binding="helper_raw=bounded(helper,22232,'d0bc72355f03fb69f0ad46911aed41eeb9b62eb0f8c3cc714df67f69e583c4fc')"
assert program.count(old_binding)==1
program=program.replace(old_binding,current_binding,1)
old_ingress_sha='799eee227e9d57fb000d75cbf018e0faf3b9bc03cd592f0405d3050479a12871'
current_ingress_sha='e8d92cce1daa93a4c684c268e1aadaae19a1446e2a219f7fee9eceec19c54fb0'
assert program.count(old_ingress_sha)==1
program=program.replace(old_ingress_sha,current_ingress_sha,1)
bound_program=program
assert bound_program.replace(current_ingress_sha,old_ingress_sha,1).replace(current_binding,old_binding,1).encode()==original
anchor="ns={'__file__':str(helper),'_AUTHENTICATED_PROGRAM':helper_raw};exec(compile(helper_raw,str(helper),'exec'),ns)"
assert program.count(anchor)==1
if not uncached:
 replacement=anchor+";_install_derived(ns)"
 program=program.replace(anchor,replacement,1)
 assert program.replace(replacement,anchor,1)==bound_program
assert bound_program.replace(current_ingress_sha,old_ingress_sha,1).replace(current_binding,old_binding,1).encode()==original
exec(compile(program,str(D/'check-source.py'),'exec'),{'__file__':str(D/'check-source.py'),'__name__':'__main__','_install_derived':namespace.get('install')})
