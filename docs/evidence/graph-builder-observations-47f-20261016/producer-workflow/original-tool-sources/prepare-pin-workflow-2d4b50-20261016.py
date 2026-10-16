from pathlib import Path
import ast,copy,difflib,hashlib,importlib.util,json,shutil,subprocess
ROOT=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang');HEAD='2d4b68d3a416cae5fba6ac08498cd56721878ba1';ROUND=Path('/Volumes/Semaprax/lane-prompts/final-2d4b-50-qualification-20261016');OLD=Path('/Volumes/Semaprax/provenance/paid-route-pin-workflow-8bb9-76-20261016');BROWSEROLD=Path('/Volumes/Semaprax/provenance/paid-route-pin-workflow-8bb9-76-resume-20261016');OUT=Path('/Volumes/Semaprax/provenance/paid-route-pin-workflow-2d4b-50-20261016')
def sha(p):
 with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def dump(p,x):p.write_text(json.dumps(x,indent=2)+'\n')
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)
assert git('rev-parse','HEAD').decode().strip()==HEAD and not git('status','--porcelain','--untracked-files=all').strip();assert not OUT.exists();OUT.mkdir();(OUT/'original-tool-sources').mkdir();(OUT/'tool-inverses').mkdir()
rec=json.loads((OLD/'skeleton-record.json').read_text());beforehead=rec['frozen_source_head'];proof={}
for name in ['strict-828f-capture-only.py','prepare-after-actual-capture.py','archive-actual-and-build-patch.py','source_inverse_composition.py','check-ready.py']:
 before=(OLD/name).read_text();after=before.replace(beforehead,HEAD).replace('exact76','exact50')
 if name=='check-ready.py':
  anchor="assert b['exit_code']==0 and not b.get('artifact_refusals') and b['results'] and all(v['exit_code']==0 for v in b['results'])"
  assert after.count(anchor)==1;after=after.replace(anchor,anchor+"\nassert all(v.get('executed') and not v.get('resource_refusal') and not v.get('observation_errors') and v.get('source_binding_status')=='unchanged' and v.get('group_settlement',{}).get('confirmed_empty') for v in b['results']), 'all four actual command groups must settle cleanly'\n")
 ast.parse(after);(OUT/'original-tool-sources'/name).write_text(before);(OUT/name).write_text(after);(OUT/'tool-inverses'/(name+'.inverse.patch')).write_text(''.join(difflib.unified_diff(after.splitlines(True),before.splitlines(True),fromfile='derived/'+name,tofile='original/'+name)));proof[name]={'before_sha256':hashlib.sha256(before.encode()).hexdigest(),'after_sha256':sha(OUT/name),'changes':'explicithead and exactround-label literals; check-ready additionally requires actualexecuted/unchanged/settledgroups for allfour; algorithms otherwiseunchanged'}
# Keep role-fixed driver exactly unchanged, never infer producer role from basename.
producer=Path('/Volumes/Semaprax/provenance/run-current-prelude-pin-producers-role-fixed-20261015.py');shutil.copyfile(producer,OUT/'original-tool-sources'/producer.name)
# Browser derivation retains original one-handle CLI observations and all historical restorations.
before=(BROWSEROLD/'capture-browser.py').read_text();after=before.replace(beforehead,HEAD).replace('final-8bb9-76-resume-qualification-20261016',ROUND.name)
anchor=" priorpath=REPO/f'docs/evidence/browser-graph-pins-35cc-20261016/actual-capture/calculator-{name}.json'";assert after.count(anchor)==1
extra=''' mainpath=REPO/f'docs/evidence/browser-graph-pins-2600e49eb/current-hosted-calculator-{name}.json';mainwire=mainpath.read_bytes().removesuffix(b'\\n');maindata=json.loads(mainwire)
 assert domain_digest(mainwire)==maindata['graph_digest'] and maindata['budget']['used_builder_bytes']==3296148
 actual_bill=d['budget']['used_builder_bytes'];actual_field=f'"used_builder_bytes":{actual_bill}'.encode();assert raw.count(actual_field)==1
 mainrestore=raw.replace(actual_field,b'"used_builder_bytes":3296148',1)
 mainfield=('"graph_digest":'+json.dumps(d['graph_digest'])).encode();assert mainrestore.count(mainfield)==1
 mainrestore=mainrestore.replace(mainfield,('"graph_digest":'+json.dumps(maindata['graph_digest'])).encode(),1)
 assert mainrestore==mainwire,'OTHER FIELD OR BYTE DIFFERENCE FROM FULL MAIN2600: stop without pin edits'
 observedpath=Path('/Volumes/Semaprax/provenance/graph-builder-observations-8bb9-20261016/browser')/f'calculator-{name}.json';observed=observedpath.read_bytes().removesuffix(b'\\n');obs=json.loads(observed)
 assert domain_digest(observed)==obs['graph_digest'] and obs['budget']['used_builder_bytes']==3534564
 observedrestore=raw.replace(actual_field,b'"used_builder_bytes":3534564',1).replace(mainfield,('"graph_digest":'+json.dumps(obs['graph_digest'])).encode(),1)
 assert observedrestore==observed,'OTHER FIELD OR BYTE DIFFERENCE FROM ACTUAL8bb: stop without pin edits'
 result['new_independent_bridges']={'main2600_wire_sha256':sha(mainpath),'main2600_builder_bytes':3296148,'observed8bb_wire_sha256':sha(observedpath),'observed8bb_builder_bytes':3534564,'complete_main2600_restore':True,'complete_observed8bb_restore':True}
'''
after=after.replace(anchor,extra+anchor,1)
# Contract source ownership follows current exact workflow, not a stale line span.
after=after.replace("'producer_contract':'.github/workflows/ci.yml:979-1005'","'producer_contract':'.github/workflows/ci.yml exact frozen2d4b / Main eb13 browser graph capture','producer_contract_sha256':sha(REPO/'.github/workflows/ci.yml')")
ast.parse(after);(OUT/'original-tool-sources/capture-browser.py').write_text(before);(OUT/'capture-browser.py').write_text(after);(OUT/'tool-inverses/capture-browser.py.inverse.patch').write_text(''.join(difflib.unified_diff(after.splitlines(True),before.splitlines(True),fromfile='derived/capture-browser.py',tofile='original/capture-browser.py')));proof['capture-browser.py']={'before_sha256':hashlib.sha256(before.encode()).hexdigest(),'after_sha256':sha(OUT/'capture-browser.py'),'changes':'exact2d4b/fullround; independent completeMain2600 and actual8bb restores beforeunchanged35cc/17104/841restores; currentCIwholeSHA'}
rec.update(frozen_source_head=HEAD,frozen_source_tree=git('rev-parse','HEAD^{tree}').decode().strip(),repository=str(ROOT),required_round=str(ROUND),required_manifest_sha256=sha(ROUND/'expected-counts.json'),required_ordered_build_commands=json.loads((ROUND/'preparation-receipt.json').read_text())['ordered_build_commands'],old_source_sha256={},adaptation='Fresh regular2d4b four-command admission. Managed immediatepublishedpins35cc1209652, strictchecker algorithms unchanged exceptfullhead; browserMain2600 strictcurrent3296148 plus completeown35cc/17104/841 and observed8bb archives. No actualnewvalue, capture orapply.')
for key in ['required_resumption_record_sha256','required_resume_driver_sha256','derived_build_admission']:rec.pop(key,None)
for name in json.loads((OLD/'skeleton-record.json').read_text())['old_source_sha256']:
 b=git('show',HEAD+':'+name);assert b==(ROOT/name).read_bytes();p=OUT/'frozen-source-before'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b);rec['old_source_sha256'][name]=sha(p)
rec['role_fixed_producer']={'path':str(OUT/'original-tool-sources'/producer.name),'sha256':sha(producer),'unchanged':True};rec['template_script_sha256']={n:sha(OUT/n)for n in proof};rec['trusted_tool_sha256']={n:sha(OUT/n)for n in proof};rec['trusted_tool_sha256']['original-tool-sources/'+producer.name]=sha(producer)
rec['strict_checker_derived_sha256']=sha(OUT/'strict-828f-capture-only.py');rec['actual_8bb_observation_manifest']={'path':'/Volumes/Semaprax/provenance/graph-builder-observations-8bb9-20261016/asset-manifest.json','sha256':sha(Path('/Volumes/Semaprax/provenance/graph-builder-observations-8bb9-20261016/asset-manifest.json'))};rec['browser_required_prior']={}
for name in ['baseline','renamed']:
 for domain,path in [('Main2600',ROOT/f'docs/evidence/browser-graph-pins-2600e49eb/current-hosted-calculator-{name}.json'),('own35cc',ROOT/f'docs/evidence/browser-graph-pins-35cc-20261016/actual-capture/calculator-{name}.json'),('observed8bb',Path(f'/Volumes/Semaprax/provenance/graph-builder-observations-8bb9-20261016/browser/calculator-{name}.json'))]:rec['browser_required_prior'][domain+'-'+name]={'path':str(path),'sha256':sha(path)}
dump(OUT/'skeleton-record.json',rec);dump(OUT/'tool-derivation.json',proof)
# Execute only unchanged finite Git/source inverse compositor, no compiler or capture.
spec=importlib.util.spec_from_file_location('sourcecomposition',OUT/'source_inverse_composition.py');mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod);projection,legacy,archives=mod.compose(ROOT,HEAD,'39130d9237121b61e40b47f95c072c09a456554e','docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json',{})
assert len(projection['rows'])==2 and sum(x['legacy_inverse_count']for x in projection['rows'])==107;dump(OUT/'raw2d4b-complete-source-projections.json',projection)
dump(OUT/'source-preparation-proof.json',{'status':'SOURCE_ONLY_PREPARED_NO_BUILD_PRODUCER_PIN_EXECUTION','source_head':HEAD,'qualification_round':str(ROUND),'manifest_sha256':rec['required_manifest_sha256'],'full_current_to391_rows':2,'all_original_inverse_edits':107,'immutable_legacy_table_sha256':hashlib.sha256(legacy).hexdigest(),'unchanged_compositor_sha256':sha(OUT/'source_inverse_composition.py'),'role_fixed_producer_byteexact':sha(producer)==sha(OUT/'original-tool-sources'/producer.name),'actual_next_values':None,'capture_execution':False,'pin_apply':False})
assert git('rev-parse','HEAD').decode().strip()==HEAD and not git('status','--porcelain','--untracked-files=all').strip()
print(json.dumps({'workflow':str(OUT),'manifest':rec['required_manifest_sha256'],'source107':'PASS','producer_byteexact':True,'capture':False,'apply':False}))
