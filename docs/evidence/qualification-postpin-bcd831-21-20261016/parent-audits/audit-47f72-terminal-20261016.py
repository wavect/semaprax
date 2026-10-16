#!/usr/bin/env python3
"""Read-only independent terminal audit. No execution of subjects or publication."""
import argparse,hashlib,json,pathlib,re,subprocess,sys
R=pathlib.Path('/Volumes/Semaprax/lane-prompts/final-47f-72-qualification-20261016');HEAD='47f80b0f7a06fde9b2388439a9c0bfe998a05af1';MANIFEST='4790a86bd1130495985deaf14d216756b272f6b1615b6ea86e3d152db95430dc'
P=argparse.ArgumentParser();P.add_argument('--out',type=pathlib.Path,default=pathlib.Path('/Volumes/Semaprax/provenance/47f-72-parent-terminal-audit-20261016.json'));a=P.parse_args()
def need(v,s):
 if not v:raise AssertionError(s)
def sha(p):
 with pathlib.Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def load(n):return json.loads((R/n).read_text())
assets={}
def digest(p,h):
 p=pathlib.Path(p);actual=sha(p);need(actual==h,'HASH '+str(p));assets[str(p)]={'sha256':actual,'bytes':p.stat().st_size};return actual
def phase(x,label):
 need(x.get('executed') is True and isinstance(x.get('exit_code'),int),label+' not terminal executed')
 need(not x.get('resource_refusal') and not x.get('observation_errors') and not x.get('timed_out'),label+' resource/observation refusal')
 need(x.get('source_binding_status')=='unchanged',label+' source')
 g=x.get('group_settlement')or{};need(g.get('confirmed_empty')is True and not any(g.get(k)for k in ['operation_failures','observation_errors','resource_refusal']),label+' group')
 need(not x.get('failed_telemetry_samples'),label+' telemetry')
 need(x.get('rss_limit_bytes')==8*1024**3 and x.get('peak_process_group_rss_bytes',0)<=x['rss_limit_bytes'],label+' RSS guard')
 need(x.get('admission_system_free_percent',0)>=30,label+' free admission')
 for key,hkey in [('observation_helper_path','observation_helper_sha256'),('phase_monitor_path','phase_monitor_sha256')]:digest(x[key],x[hkey])
m=load('compilation-manifest.json');prep=load('preparation-receipt.json');b=load('owning-build-result.json');runtime=load('owning-selected-results.json');repo=pathlib.Path(m['draft_repository'])
need(m['source_subject']==HEAD and len(m['checks'])==72 and m['expected_groups_authored']==72 and m['expected_bodies_authored']==72,'manifest subject/count')
digest(R/'compilation-manifest.json',MANIFEST);digest(R/'expected-counts.json',MANIFEST)
need(prep['source_subject']==HEAD and prep['source_tree']==m['source_tree'] and prep['manifest_sha256']==MANIFEST and prep['selected_bodies']==72,'preparation')
for n,h in prep['driver_sha256'].items():digest(R/'drivers'/n,h)
for n,h in m['inputs_sha256'].items():digest(n,h)
# One git reader with bounded buffers verifies the immutable Git source and
# current files independently; no checkout, source write or artifact execution.
need(subprocess.check_output(['git','rev-parse',HEAD+'^{tree}'],cwd=repo,text=True).strip()==m['source_tree'],'Git tree')
git=subprocess.Popen(['git','cat-file','--batch'],cwd=repo,stdin=subprocess.PIPE,stdout=subprocess.PIPE)
for source,h in m['source_file_sha256'].items():
 digest(repo/source,h);git.stdin.write((HEAD+':'+source+'\n').encode());git.stdin.flush();header=git.stdout.readline().split();need(len(header)==3 and header[1]==b'blob','Git blob '+source);left=int(header[2]);hs=hashlib.sha256()
 while left:
  chunk=git.stdout.read(min(left,1024*1024));need(bool(chunk),'Git short read');hs.update(chunk);left-=len(chunk)
 need(git.stdout.read(1)==b'\n' and hs.hexdigest()==h,'Git SHA '+source)
git.stdin.close();need(git.wait()==0,'Git reader')
need(b['source_subject']==HEAD and b['source_tree']==m['source_tree'] and b['manifest_sha256']==MANIFEST and b['ordered_commands']==prep['ordered_build_commands'] and len(b['results'])==5 and b['exit_code']==0 and b['executed_test_bodies']==0 and not b['artifact_refusals'],'build binding/all5')
need(b['driver_sha256']==prep['driver_sha256']['build-owning.py'],'build driver')
digest(R/'owning-build.jsonl',b['owning_build_metadata_sha256']);digest(R/'owning-build.log',b['log_sha256']);digest(R/'memory-samples.jsonl',b['memory_samples_sha256'])
for i,x in enumerate(b['results']):phase(x,'build'+str(i));need(x['command']==b['ordered_commands'][i] and x['exit_code']==0,'build command'+str(i))
metadata=[json.loads(s)for s in (R/'owning-build.jsonl').read_text().splitlines()if s.strip()];artifacts={}
for x in b['artifacts']:
 digest(x['executable'],x['executable_sha256']);need(any(v.get('reason')=='compiler-artifact' and v.get('executable')==x['executable'] and v.get('package_id')==x['package_id'] and v.get('target')==x['target'] for v in metadata),'artifact metadata '+x['executable']);artifacts[x['executable']]=x['executable_sha256']
need(runtime['source_subject']==HEAD and runtime['complete']is True and not runtime.get('final_binding_refusal') and len(runtime['results'])==72,'runtime terminal')
outcomes=[]
for i,(check,x)in enumerate(zip(m['checks'],runtime['results'])):
 label='runtime'+str(i);phase(x,label);need(x['source_subject']==HEAD and x['selector']==check['selector'] and x['harness']==check['package']+'::'+check['harness'].split(':')[-1],label+' ordered tuple')
 need(x['runtime_manifest_sha256']==MANIFEST and x['owning_build_receipt_sha256']==sha(R/'owning-build-result.json') and x['owning_build_metadata_sha256']==b['owning_build_metadata_sha256'] and x['driver_sha256']==prep['driver_sha256']['run-owning-selected.py'],label+' receipt/driver')
 need(x['executable_sha256_before']==x['executable_sha256_after']==x['executable_sha256']==artifacts.get(x['executable']),label+' executable')
 digest(x['log'],x['log_sha256']);digest(x['memory_samples_path'],x['memory_samples_sha256'])
 samples=[json.loads(s)for s in pathlib.Path(x['memory_samples_path']).read_text().splitlines()if s.strip()];need(bool(samples),label+' no samples')
 need(x['expected_selected_static']==1 and x['discovered_names']==[check['selector']],label+' exact discovery')
 summary=x['summary'];need(len(summary)==4 and int(summary[1])+int(summary[2])==1 and int(summary[3])==0,label+' body summary')
 text=pathlib.Path(x['log']).read_text();matches=re.findall(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text);need(matches==[tuple(summary)],label+' raw summary mismatch')
 need('test '+check['selector']+' ...' in text,label+' raw selector')
 passed=x['exit_code']==0;need((int(summary[1]),int(summary[2]))==((1,0)if passed else(0,1)),label+' outcome')
 outcomes.append({'index':i,'selector':x['selector'],'harness':x['harness'],'status':'PASS'if passed else'FAIL','exit_code':x['exit_code'],'summary':summary,'log_sha256':x['log_sha256'],'memory_samples_sha256':x['memory_samples_sha256'],'confirmed_empty':True,'resource_refusal':None})
passed=sum(x['status']=='PASS'for x in outcomes);failed=72-passed;need((passed,failed)==(56,16),'actual counts are not56/16')
clippy=[]
for name in ['clippy-production-result.json','clippy-production-toolchain-full-cli-result.json']:
 path=R/name
 if not path.exists():clippy.append({'receipt':str(path),'status':'PENDING_NOT_CLAIMED'});continue
 c=load(name);need(c['source_subject']==HEAD and c['manifest_sha256']==MANIFEST,'Clippy binding');phase(c,name);digest(c['log'],c['log_sha256']);digest(c['memory_samples_path'],c['memory_samples_sha256']);clippy.append({'receipt':str(path),'receipt_sha256':sha(path),'status':'TERMINAL','exit_code':c['exit_code'],'confirmed_empty':True})
# Parent audit completeness covers actual72 runtime, not unfinished lint.
result={'source':HEAD,'source_subject':HEAD,'complete':True,'actual':72,'PASS':passed,'FAIL':failed,'all_logs_authenticated':True,'all_memory_samples_authenticated':True,'all_groups_confirmed_empty':True,'resource_or_observation_refusals':[],'source_tree':m['source_tree'],'manifest_sha256':MANIFEST,'source_files_git_and_worktree_authenticated':len(m['source_file_sha256']),'all_five_build_commands_pass':True,'build_commands':5,'artifact_count':len(artifacts),'build_receipt_sha256':sha(R/'owning-build-result.json'),'runtime_receipt_sha256':sha(R/'owning-selected-results.json'),'artifact_metadata_sha256':b['owning_build_metadata_sha256'],'artifact_live_hashes_authenticated':True,'clippy':clippy,'both_clippy_terminal':all(c['status']=='TERMINAL'for c in clippy),'both_clippy_pass':all(c.get('exit_code')==0 for c in clippy),'clippy_pending_not_success':any(c['status']!='TERMINAL'for c in clippy),'failures_preserved':[x for x in outcomes if x['status']=='FAIL'],'outcomes':outcomes,'authenticated_assets':assets,'audit_script':str(pathlib.Path(__file__).resolve()),'audit_script_sha256':sha(__file__),'round_read_only':True,'publication_performed':False,'universal_issue_closure_authorized':False,'history_scopes_not_relabelled':'Current72=56/16. Earlier364d60=52/8;2d4b50=39/11;8bb76=56/20;separate9=9/0;175=135/40 remain distinct in original nested histories. Assembler authenticates them independently; this audit never promotes omitted owners.'}
a.out.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'status':'PASS_READONLY_TERMINAL_RUNTIME_AUDIT','output':str(a.out),'sha256':sha(a.out),'actual':72,'PASS':passed,'FAIL':failed,'source_files':len(m['source_file_sha256']),'artifacts':len(artifacts),'clippy':clippy}))
