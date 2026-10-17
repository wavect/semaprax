"""One finite branch-test generation, preserving the complete original strict binder.
Execution requires an explicitly reviewed full HEAD. No compiler is invoked.
"""
from pathlib import Path
import hashlib,os,stat
D=Path(__file__).resolve().parent
REL='docs/evidence/compile4-exact-source-successor-20261017/frozen174/'
OLD='docs/evidence/main09013-source-namespaces-v1/bind_current.py'
BEFORE_LEN=241192
BEFORE_SHA='c86997c1e82c046da48e4176a3724e65089d2e9236b9724ab8e5ec7b93a95cc1'
BINDER_LEN=13491
BINDER_SHA='4921dedbcd93d865cafc825f176f5598891864f35c605096185dfb49c6887546'
CHANGED={'src/cleanup_plan/replay/branch_clone.rs': (17063, 'dbedf701561e84f3bad64dca84478115c28884fb3ff12b9aa5b34b3ed4ae5274'), 'src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs': (37266, 'a3aec4ec0190ac665d563ec9c231e4120c073b0e30e1647ad760a92a35e2e44b')}

sha=lambda b:hashlib.sha256(b).hexdigest()
def held(path,length,digest):
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 identity=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode)and first.st_size==length<=262144
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(length);assert len(b)==length and not f.read(1)
  assert identity(first)==identity(os.fstat(fd))==identity(path.lstat())and sha(b)==digest
  return b
 finally:os.close(fd)
def guard_physical(m,physical):
 prior={x['path']:(x['current_len'],x['current_sha256'])for x in m['physical']}
 assert len(prior)==174 and set(prior)==set(physical)
 assert set(CHANGED)<=set(prior)
 for path,pair in physical.items():
  assert pair==(CHANGED[path]if path in CHANGED else prior[path]),('foreign next-generation physical input',path)
def render(raw):
 assert len(raw)==BINDER_LEN and sha(raw)==BINDER_SHA
 text=raw.decode()
 edits=[
  ("before=current(d['manifest']);require(before==template,'one-shot binder requires exact original template')",
   "before=current(d['manifest']);require(len(before)==NEXT_BEFORE_LEN and sha(before)==NEXT_BEFORE_SHA,'finite next binder requires exact retained complete preceding generation')\n require(current(NEXT_REL+'input-domains.before.json')==before,'retained prior generation drift')\n require(current(NEXT_REL+'bind_current.before.py.txt')==current(d['binder']),'whole original binder drift')\n current(NEXT_REL+'bind-next.py')"),
  ("m=parse(template);require(m['table_head']", "m=parse(before);require(m['table_head']"),
  ("require(set(row['path'] for row in m['main'])<=set(physical),'missing physical domain')",
   "require(set(row['path'] for row in m['main'])<=set(physical),'missing physical domain')\n next_guard_physical(m,physical)"),
 ]
 for before,after in edits:assert text.count(before)==1;text=text.replace(before,after,1)
 restored=text
 for before,after in reversed(edits):assert restored.count(after)==1;restored=restored.replace(after,before,1)
 assert restored.encode()==raw
 return render_raw_guard_successor(text)

LEGACY={'docs/evidence/main043-independent-custody-20261016/guard-extension-allowed.json': {'actual': (62474, '9bd9fe6bf24f8030f2307dafd423a6bd678e745e5ef67b75465c585ca258ece3'), 'archive': 'docs/evidence/main090-accepted174-final-binding-20261017/legacy-0.txt', 'prior': (62474, '9bd9fe6bf24f8030f2307dafd423a6bd678e745e5ef67b75465c585ca258ece3')}, 'tests/hir_module_boundaries.rs': {'actual': (99079, '7dade8fc55a0ea5f4a16bc57d1ac8ce43ed200694a39ea6e3d33038228c8b8c1'), 'archive': 'docs/evidence/main090-accepted174-final-binding-20261017/legacy-1.txt', 'prior': (98174, '5090d08b52cb9650af5fc2c2a28739a9783c3d43c7a0f25037ce032b04f51137')}, 'tests/workspace/semantic_graph/source_inventory.rs': {'actual': (94257, '8debd7324052ca398a9385bdf7b47f1b479d8638b8e5ed9dc72ef4b9213cd177'), 'archive': 'docs/evidence/main090-accepted174-final-binding-20261017/legacy-2.txt', 'prior': (91318, 'aa4e1844d2e867c519bd24fdb560535c9f07b7daa790e343c1b4b04e978d0989')}, 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs': {'actual': (157441, '369a29b5860f644b33b82e8e968ab66225faf70cfb930757aff04ba591824d4e'), 'archive': 'docs/evidence/main090-accepted174-final-binding-20261017/legacy-3.txt', 'prior': (156404, 'bdf614306dd818a9d66a794edf46008c3306ffa2fabcb3acbf4b03060d7db316')}, 'docs/evidence/main043-independent-custody-20261016/guard-extension-bindings/tests__hir_module_boundaries.rs.json': {'actual': (11472, '025de3da8662e2c94b4427cc3779bd3b665f923893cb95b961f74c89b27dfd8c'), 'archive': 'docs/evidence/main090-accepted174-final-binding-20261017/legacy-4.txt', 'prior': (11472, '025de3da8662e2c94b4427cc3779bd3b665f923893cb95b961f74c89b27dfd8c')}, 'docs/evidence/main043-independent-custody-20261016/guard-extension-bindings/tests__workspace__semantic_graph__source_inventory.rs.json': {'actual': (14269, '4adea0c6da5535355ea6374630eb139c606b84bed4bd66ac0fdf95b9a81b3102'), 'archive': 'docs/evidence/main090-accepted174-final-binding-20261017/legacy-5.txt', 'prior': (14269, '4adea0c6da5535355ea6374630eb139c606b84bed4bd66ac0fdf95b9a81b3102')}, 'docs/evidence/main043-independent-custody-20261016/guard-extension-bindings/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.json': {'actual': (13688, 'dc2907e379bbb45e1e0ba51000545184d6e650438567b52991c1172db752b433'), 'archive': 'docs/evidence/main090-accepted174-final-binding-20261017/legacy-6.txt', 'prior': (13688, 'dc2907e379bbb45e1e0ba51000545184d6e650438567b52991c1172db752b433')}}
STRICT={'docs/evidence/accepted-strict-guard-generation-20261017/guard_payload.py': (3965, '17d849f39380f26dcd0d12c9c3754f7480d8d3153a44a20a31d849901066e147'), 'docs/evidence/main043-independent-custody-20261016/guard-extensions.py': (5113, 'ccfb62c8b30e5564446ddc6eb2a31f5a7502e5ed43d3d5077a8c0105c42ef95a'), 'docs/evidence/accepted-strict-guard-generation-20261017/before/guard-extensions.py': (5113, 'ccfb62c8b30e5564446ddc6eb2a31f5a7502e5ed43d3d5077a8c0105c42ef95a'), 'docs/evidence/accepted-strict-guard-generation-20261017/current-allowed.json': (70162, '0969a237be8a596d0b469f8da25aa5e99f88c6190b46c263abf69f048a14e5a7'), 'docs/evidence/main043-independent-custody-20261016/guard-extension-original/verify-or-refresh.py': (7938, 'b8b72537f880da2655f102002275dbe0c5a7eb0aee72982eae017204d15266c7'), 'docs/evidence/main043-independent-custody-20261016/guard-extension-recipe-edits.json': (37966, '95d941061f015386674d81ec51afbd3ca17d6aaa198639a2617dfb50851f3542'), 'docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py': (2713, '340f54c3e919b2690b089f090a47876a141efca094bbb6589e96026d61d51d90'), 'docs/evidence/accepted-strict-guard-generation-20261017/current-bindings/tests__hir_module_boundaries.rs.json': (12399, 'a99febda8d720446292b504dfa7cd9af95fb40e2705f88580b42be78a5c19d63'), 'docs/evidence/accepted-strict-guard-generation-20261017/current-bindings/tests__workspace__semantic_graph__source_inventory.rs.json': (17315, 'a98297ee4d1f161e0484680c22168253b86050243845d168e02b7a55f6b4935b'), 'docs/evidence/accepted-strict-guard-generation-20261017/current-bindings/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.json': (14747, '2bc1821ded60594e5c2ea2a4b0553e36975776dfb43c7f195c9b49689eaee060'), 'tests/hir_module_boundaries.rs': (99079, '7dade8fc55a0ea5f4a16bc57d1ac8ce43ed200694a39ea6e3d33038228c8b8c1'), 'tests/workspace/semantic_graph/source_inventory.rs': (94257, '8debd7324052ca398a9385bdf7b47f1b479d8638b8e5ed9dc72ef4b9213cd177'), 'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs': (157441, '369a29b5860f644b33b82e8e968ab66225faf70cfb930757aff04ba591824d4e')}
STRICT_HELPER='docs/evidence/accepted-strict-guard-generation-20261017/guard_payload.py'
OLD_RAW_GUARD='def raw_guards(descriptor):\n allowed_path=descriptor[\'allowed_path\'];allowed=parse(current(allowed_path));baseline=descriptor[\'prior_allowed\']\n require(set(allowed)==set(baseline),\'foreign guard carriers\')\n for carrier in descriptor[\'raw3\']:\n  path=carrier[\'path\'];raw=current(path).decode();old=git(\'show\',descriptor[\'raw_original_head\']+\':\'+path)\n  require(sha(old)==carrier[\'original_sha256\'],\'original raw scope drift\');original=old.decode()\n  additions=fragments(path,raw,original);base=PurePosixPath(path).parent if path==\'tests/hir_module_boundaries.rs\' else PurePosixPath(path).parent.parent\n  counts=Counter()\n  for added in additions:\n   for literal in re.findall(r\'include_str!\\("([^"\\n]+)"\\)\',added):\n    joined=os.path.normpath(str(base/literal));parts(joined);counts[joined]+=1\n  rule=allowed[path]\n  require(set(rule[\'required\'])<=set(rule[\'maximum\']),\'required guard subjects lack maxima\')\n  for kind in (\'required\',\'maximum\'):\n   for k,v in baseline[path][kind].items():require(rule[kind].get(k)==v,\'prior guard count changed: \'+path+\' \'+k)\n  require(set(counts)<=set(rule[\'maximum\']),\'foreign raw include\')\n  require(all(rule[\'required\'].get(k,0)<=counts[k]<=v for k,v in rule[\'maximum\'].items()),\'missing/duplicate raw source\')\n  owner=descriptor[\'module\'];require(counts[owner]==rule[\'required\'][owner]==rule[\'maximum\'][owner]==1,\'namespace must appear exactly once in each original scope\')\n  if \'ledger_capacity\' in path:require(len(raw.splitlines())<=1500,\'native source inventory line bound\')\n  binding_path=descriptor[\'guard_directory\']+\'/guard-extension-bindings/\'+path.replace(\'/\',\'__\')+\'.json\'\n  expected=dict(source=path,current_sha256=sha(raw.encode()),original_sha256=sha(old),added_fragments=additions)\n  require(parse(current(binding_path))==expected,\'stale raw scope binding: \'+path)\n'
NEW_RAW_GUARD='def legacy_raw_guards(descriptor, physical_current):\n def current(path):return accepted_legacy_current(path,physical_current)\n allowed_path=descriptor[\'allowed_path\'];allowed=parse(current(allowed_path));baseline=descriptor[\'prior_allowed\']\n require(set(allowed)==set(baseline),\'foreign guard carriers\')\n for carrier in descriptor[\'raw3\']:\n  path=carrier[\'path\'];raw=current(path).decode();old=git(\'show\',descriptor[\'raw_original_head\']+\':\'+path)\n  require(sha(old)==carrier[\'original_sha256\'],\'original raw scope drift\');original=old.decode()\n  additions=fragments(path,raw,original);base=PurePosixPath(path).parent if path==\'tests/hir_module_boundaries.rs\' else PurePosixPath(path).parent.parent\n  counts=Counter()\n  for added in additions:\n   for literal in re.findall(r\'include_str!\\("([^"\\n]+)"\\)\',added):\n    joined=os.path.normpath(str(base/literal));parts(joined);counts[joined]+=1\n  rule=allowed[path]\n  require(set(rule[\'required\'])<=set(rule[\'maximum\']),\'required guard subjects lack maxima\')\n  for kind in (\'required\',\'maximum\'):\n   for k,v in baseline[path][kind].items():require(rule[kind].get(k)==v,\'prior guard count changed: \'+path+\' \'+k)\n  require(set(counts)<=set(rule[\'maximum\']),\'foreign raw include\')\n  require(all(rule[\'required\'].get(k,0)<=counts[k]<=v for k,v in rule[\'maximum\'].items()),\'missing/duplicate raw source\')\n  owner=descriptor[\'module\'];require(counts[owner]==rule[\'required\'][owner]==rule[\'maximum\'][owner]==1,\'namespace must appear exactly once in each original scope\')\n  if \'ledger_capacity\' in path:require(len(raw.splitlines())<=1500,\'native source inventory line bound\')\n  binding_path=descriptor[\'guard_directory\']+\'/guard-extension-bindings/\'+path.replace(\'/\',\'__\')+\'.json\'\n  expected=dict(source=path,current_sha256=sha(raw.encode()),original_sha256=sha(old),added_fragments=additions)\n  require(parse(current(binding_path))==expected,\'stale raw scope binding: \'+path)\n\ndef raw_guards(descriptor):\n accepted_strict_raw_guards(descriptor,current,git,require)\n return legacy_raw_guards(descriptor,current)\n'

def accepted_legacy_current(path,physical_current):
 assert path in LEGACY, ('foreign legacy guard acquisition',path)
 row=LEGACY[path];actual=physical_current(path)
 assert (len(actual),sha(actual))==row['actual'],('actual legacy-channel drift',path)
 prior=physical_current(row['archive'])
 assert (len(prior),sha(prior))==row['prior'],('whole legacy archive drift',path)
 return prior

def accepted_strict_raw_guards(descriptor,physical_current,git,require):
 # All executable/dependency bytes first pass ORIGINAL174 bounded nofollow
 # root-FD/Git/current acquisition. No Path/current global override is installed.
 held={}
 for path,pair in STRICT.items():
  raw=physical_current(path)
  require((len(raw),sha(raw))==pair,'strict guard dependency drift: '+path)
  held[path]=raw
 namespace={'__file__':str(D.parents[3]/STRICT_HELPER),'__name__':'accepted174_actual_strict_guard'}
 exec(compile(held[STRICT_HELPER],namespace['__file__'],'exec'),namespace)
 actual=namespace['accepted_guard_extensions'](D.parents[3])
 actual['verify_recipe']()
 for carrier in descriptor['raw3']:
  path=carrier['path'];original=git('show',descriptor['raw_original_head']+':'+path)
  actual['verify_guard'](path,held[path].decode(),original.decode(),False)

def render_raw_guard_successor(text):
 assert text.count(OLD_RAW_GUARD)==1
 rendered=text.replace(OLD_RAW_GUARD,NEW_RAW_GUARD,1)
 assert rendered.replace(NEW_RAW_GUARD,OLD_RAW_GUARD,1)==text
 return rendered

def main():
 raw=held(D/'bind_current.before.py.txt',BINDER_LEN,BINDER_SHA)
 held(D/'input-domains.before.json',BEFORE_LEN,BEFORE_SHA)
 # Complete original CLI, root FD reader, descriptor, namespace mask, old
 # template, raw3 parser, 174 Git acquisitions, race rechecks, O_EXCL archive,
 # staging and publication run from authenticated held code without reopen.
 exec(compile(render(raw),str(D/ 'bind_current.before.py.txt'),'exec'),{
  '__name__':'__main__','__file__':str(D/'bind_current.before.py.txt'),
  'NEXT_REL':REL,'NEXT_BEFORE_LEN':BEFORE_LEN,'NEXT_BEFORE_SHA':BEFORE_SHA,
  'next_guard_physical':guard_physical,
  'accepted_legacy_current':accepted_legacy_current,
  'accepted_strict_raw_guards':accepted_strict_raw_guards,
 })
if __name__=='__main__':main()
