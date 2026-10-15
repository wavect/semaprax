from pathlib import Path
import sys,json,hashlib,subprocess,os,time,re
BUILD=Path("/Volumes/Semaprax/lane-prompts/final-digest-borrow-qualification-20261011")
OLD=Path('/Volumes/Semaprax/lane-prompts/pin-consumer-d8fad-seven-20261015')
sys.path.insert(0,str(OLD/'drivers'))
from observed_phase import run_phase,provenance
OUT=Path(__file__).resolve().parent
ROOT=Path('/Volumes/Semaprax/worktrees/closure-767-6548-20261015/AI-Lang')
CURRENT=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang')
HEAD='654846173bb90874779dfee44ab0832c03715e84'
sha=lambda p:hashlib.file_digest(Path(p).open('rb'),'sha256').hexdigest()
receipt=BUILD/'owning-build-result.json';build=json.loads(receipt.read_text())
assert build['source_subject']==HEAD and build['exit_code']==0 and all(r['exit_code']==0 for r in build['results'])
artifact=[a for a in build['artifacts'] if a['target']['name']=='language'][0]
EXE=Path(artifact['executable']);EXESHA=artifact['executable_sha256'];assert sha(EXE)==EXESHA
# No whole-current owning-cut claim; complete cut audit is separate from this actual old-subject execution.
cut=[]
assert sha(BUILD/"owning-build.jsonl")==build["owning_build_metadata_sha256"]
assert sha(BUILD/"owning-build.log")==build["log_sha256"]
selectors=["borrowed_str_equality::borrowed_str_equality_source_graph_and_hostile_modes","borrowed_str_equality::borrowed_str_equality_interpreter_native_bytes_and_late_failure","borrowed_str_equality::borrowed_str_equality_public_text_validates_both_roots_before_entry"]
manifest={'source_subject':HEAD,'source_tree':build['source_tree'],'build_receipt_sha256':sha(receipt),'build_metadata_sha256':sha(BUILD/'owning-build.jsonl'),'executable':str(EXE),'executable_sha256':EXESHA,'selectors':selectors,'no_whole_current_source_equality_claim':True,'controller_sha256':sha(__file__),'guard':provenance(),'no_cargo':True,'no_current_999fa_whole_compiler_qualification':True}
(OUT/'binding.json').write_text(json.dumps(manifest,indent=2)+'\n')
def bound():
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==HEAD
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip()
 assert sha(EXE)==EXESHA and sha(receipt)==manifest['build_receipt_sha256']
bound();env=os.environ.copy();tmp=OUT/'runtime-tmp';tmp.mkdir(exist_ok=True);env['TMPDIR']=str(tmp)
env["PATH"]="/Users/kevin/.nvm/versions/node/v24.3.0/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"
versions=[]
for name in ["clang","node"]:
 v=subprocess.run([name,"--version"],cwd=ROOT,env=env,text=True,capture_output=True)
 assert v.returncode==0,(name,v.stderr)
 versions.append({"command":[name,"--version"],"returncode":v.returncode,"stdout":v.stdout,"stderr":v.stderr})
manifest["backend_availability"]=versions
manifest["backend_PATH"]=env["PATH"]
(OUT/"binding.json").write_text(json.dumps(manifest,indent=2)+"\n")
results=[]
for i,s in enumerate(selectors):
 bound();listed=subprocess.run([str(EXE),s,'--list','--format','terse','--exact'],cwd=ROOT,env=env,text=True,capture_output=True)
 assert listed.returncode==0 and [l[:-6] for l in listed.stdout.splitlines() if l.endswith(': test')]==[s]
 log=OUT/f'{i}.log';samples=OUT/f'{i}-memory.jsonl';assert not log.exists() and not samples.exists()
 def emit(row):
  with samples.open('a') as f:f.write(json.dumps(row)+'\n')
 with log.open('x') as f:
  result=run_phase([str(EXE),s,'--exact','--nocapture','--test-threads=1'],cwd=ROOT,env=env,stdout=f,stderr=subprocess.STDOUT,rss_limit_bytes=8*1024**3,emit=emit,wall_limit=600,wall_deadline_inclusive=True,bind=bound,command_index=i)
 summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',log.read_text())
 result.update(selector=s,source_subject=HEAD,log_sha256=sha(log),summary=summaries[-1] if summaries else None,executable_sha256_after=sha(EXE))
 results.append(result);(OUT/'results.json').write_text(json.dumps({'source_subject':HEAD,'complete':len(results)==len(selectors),'results':results},indent=2)+'\n');print(s,result['exit_code'],result['summary'],flush=True)
 if result['resource_refusal']:break
bound()
assert len(results)==len(selectors) and all(r['exit_code']==0 and r['summary']==('ok','1','0','0') for r in results)
