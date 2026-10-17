"""Run unchanged strict raw-guard predicates against every new physical scope."""
from pathlib import Path
import importlib.util,json,re,subprocess,tempfile,hashlib
D=Path(__file__).resolve().parent;R=D.parents[2]
spec=importlib.util.spec_from_file_location('original_guard',R/'docs/evidence/main043-independent-custody-20261016/guard-extensions.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
allowed=(g.D/'guard-extension-allowed.json').read_bytes()
subjects=['src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs','src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs','src/source_reflows/incoming21_semantic_receivers.rs']
rows=[]
with tempfile.TemporaryDirectory(prefix='main21-raw-scopes-',dir='/Volumes/Semaprax/runtime-tmp') as temporary:
 g.D=Path(temporary);(g.D/'guard-extension-allowed.json').write_bytes(allowed)
 for path in json.loads(allowed):
  raw=(R/path).read_text();old=subprocess.check_output(['git','show','a82e00cc8:'+path],cwd=R,text=True)
  g.verify_guard(path,raw,old,refresh=True);g.verify_guard(path,raw,old)
  base=(R/path).parent if path=='tests/hir_module_boundaries.rs' else (R/path).parent.parent
  found={s:[] for s in subjects}
  for match in re.finditer(r'include_str!\("([^"\n]+)"\),',raw):
   logical=str((base/match[1]).resolve().relative_to(R))
   if logical in found:found[logical].append(match)
  scopes=2 if path=='tests/hir_module_boundaries.rs' else 1 if path.startswith('tests/workspace/') else 3
  assert [len(found[s]) for s in subjects]==[scopes,scopes,1]
  for subject,matches in found.items():
   for ordinal,match in enumerate(matches):
    token=match[0]
    mutants={'unknown':raw[:match.start()]+token.replace(match[1],'unknown-main21-subject.rs')+raw[match.end():],
      'duplicate':raw[:match.end()]+' '+token+raw[match.end():]}
    for kind,mutant in mutants.items():
     try:g.verify_guard(path,mutant,old)
     except (AssertionError,ValueError):pass
     else:raise AssertionError((path,subject,ordinal,kind,'accepted'))
    rows.append({'carrier':path,'subject':subject,'scope_ordinal':ordinal,'unknown_refused':True,'duplicate_refused':True})
print(json.dumps({'status':'PASS_PURE_SOURCE_ORIGINAL_RAW_GUARD','matching_scopes_hir_workspace_native':[2,1,3],'namespace_scopes':[1,1,1],'occurrences':len(rows),'mutations_refused':2*len(rows),'checks':rows,'rust_executed':False},indent=2))
