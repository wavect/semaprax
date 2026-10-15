from pathlib import Path
import sys,json,hashlib,subprocess,os,time,re
OLD=Path('/Volumes/Semaprax/lane-prompts/pin-consumer-d8fad-seven-20261015')
sys.path.insert(0,str(OLD/'drivers'))
from observed_phase import run_phase,provenance
OUT=Path(__file__).resolve().parent
ROOT=Path('/Volumes/Semaprax/worktrees/closure-d8fad-three-20261015/AI-Lang')
CURRENT=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang')
HEAD='d8fadfb7cd5ece54504cda542575e5a154d3657c'
sha=lambda p:hashlib.file_digest(Path(p).open('rb'),'sha256').hexdigest()
receipt=OLD/'owning-build-result.json';build=json.loads(receipt.read_text())
assert build['source_subject']==HEAD and build['exit_code']==0 and all(r['exit_code']==0 for r in build['results'])
artifact=[a for a in build['artifacts'] if a['target']['kind']==['lib']][0]
EXE=Path(artifact['executable']);EXESHA=artifact['executable_sha256'];assert sha(EXE)==EXESHA
paths=['src/hir/validation.rs', 'src/hir/validation/contract_owners.rs', 'src/hir/validation/contract_owners', 'src/hir/validation/scope_checkpoint.rs', 'src/hir/validation/scope_checkpoint', 'src/hir/validation/function_scope.rs', 'src/hir/validation/function_scope', 'src/hir/validation/generic_template/values.rs', 'src/hir/validation/generic_template/values']
# Exact ownership cut is supplementary evidence, never a whole-current-head qualification.
cut=[]
for p in paths:
 a=subprocess.check_output(['git','ls-tree','-r',HEAD,'--',p],cwd=CURRENT)
 b=subprocess.check_output(['git','ls-tree','-r','6444d6d950bc01be03720c33ad6cb175e62e8cc5','--',p],cwd=CURRENT)
 assert a==b,(p,'ownership cut changed')
 cut.append({'path':p,'tree_listing_sha256':hashlib.sha256(a).hexdigest()})
selectors=['hir::validation::function_scope::tests::production_chain_observes_actual_backing_debits_complete_drop_and_exact_short_sticky']
manifest={'source_subject':HEAD,'source_tree':build['source_tree'],'build_receipt_sha256':sha(receipt),'build_metadata_sha256':sha(OLD/'owning-build.jsonl'),'executable':str(EXE),'executable_sha256':EXESHA,'selectors':selectors,'ownership_cut_equal_to_6444':cut,'controller_sha256':sha(__file__),'guard':provenance(),'no_cargo':True,'no_current_6444_whole_compiler_qualification':True}
(OUT/'binding.json').write_text(json.dumps(manifest,indent=2)+'\n')
def bound():
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==HEAD
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip()
 assert sha(EXE)==EXESHA and sha(receipt)==manifest['build_receipt_sha256']
bound();env=os.environ.copy();tmp=OUT/'runtime-tmp';tmp.mkdir(exist_ok=True);env['TMPDIR']=str(tmp)
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
