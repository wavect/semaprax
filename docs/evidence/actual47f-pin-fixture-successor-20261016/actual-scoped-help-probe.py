from pathlib import Path
import hashlib,json,re,subprocess,tempfile,os
ROOT=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang');FIX=Path('/Volumes/Semaprax/worktrees/alias-owned-profile-fixture-47f-20261016/AI-Lang');ROUND=Path('/Volumes/Semaprax/lane-prompts/final-47f-72-qualification-20261016');OUT=Path('/Volumes/Semaprax/provenance/scoped-help-wire-probes-47f-v2-20261016');OUT.mkdir(exist_ok=False);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();b=json.loads((ROUND/'owning-build-result.json').read_text());assert b['exit_code']==0 and len(b['results'])==5;assert sha(ROUND/'owning-build.jsonl')==b['owning_build_metadata_sha256'];roles={}
for line in (ROUND/'owning-build.jsonl').open():
 try:v=json.loads(line)
 except:continue
 if v.get('reason')=='compiler-artifact'and v.get('executable')and v['target']['kind']==['bin']and not v['profile']['test']and v['target']['name']in ['semaprax','semaprax-full']:
  key=v['target']['name'];row=(v['executable'],v['profile'],v['target']);assert key not in roles or roles[key]==row;roles[key]=row
results=[];bind={};
for role,path in [('semaprax','tests/cli_help_surface_v1.rs'),('semaprax-full','crates/semaprax-toolchain/tests/cli_help_surface_v1.rs')]:
 exe=Path(roles[role][0]);exe_sha=sha(exe);bind[role]={'executable':str(exe),'sha256':exe_sha,'expected_fixture_source':path,'expected_fixture_sha256':sha(FIX/path)};expected_help=json.loads(re.search(r'const JSON_CODEC_HELP: &str = ("(?:[^"\\]|\\.)*");',(FIX/path).read_text())[1]);n=0
 def invoke(args):
  global n
  assert sha(exe)==exe_sha
  with tempfile.TemporaryDirectory(prefix='help47-',dir='/Volumes/Semaprax/tmp')as cwd:
   p=subprocess.run([str(exe),*args],cwd=cwd,capture_output=True,timeout=10);assert list(Path(cwd).iterdir())==[]
  stem=f'{role}-{n:03}';n+=1;(OUT/(stem+'.stdout')).write_bytes(p.stdout);(OUT/(stem+'.stderr')).write_bytes(p.stderr);results.append({'role':role,'arguments':args,'exit_code':p.returncode,'stdout':stem+'.stdout','stdout_sha256':sha(OUT/(stem+'.stdout')),'stderr':stem+'.stderr','stderr_sha256':sha(OUT/(stem+'.stderr')),'empty_working_directory_unchanged':True});return p
 catalog=invoke(['help','all']);assert catalog.returncode==0 and not catalog.stderr;text=catalog.stdout.decode();usages=[line.strip()[len('semaprax '):]for line in text.splitlines()if line.strip().startswith('semaprax ')];commands=sorted({u.split()[0]for u in usages})
 for command in commands:
  prefix='semaprax '+command;expected='Usage:\n'+''.join('  '+line.strip()+'\n'for line in text.splitlines()if line.strip()==prefix or line.strip().startswith(prefix+' '))+(expected_help if command=='json-codec'else'')
  if command=='new':expected+='  semaprax-full new <fresh-directory> --from <project> --codec-plan <plan-json> --command-profile <exact-profile> [--command-path <declared-path> --command-source <source-file>]\n'
  for args in [['help',command],[command,'--help'],[command,'-h']]:
   p=invoke(args);assert p.returncode==0 and not p.stderr and p.stdout==expected.encode(),(role,args)
 assert sha(exe)==exe_sha
print('PASS actual source-bound CLI probes',len(results),'(not Rust test bodies)');(OUT/'receipt.json').write_text(json.dumps({'source_subject':b['source_subject'],'build_receipt_sha256':sha(ROUND/'owning-build-result.json'),'raw_Cargo_metadata_sha256':b['owning_build_metadata_sha256'],'bindings':bind,'results':results,'all_actual_scoped_commands_three_spellings_exact':True,'Rust_test_bodies_executed':0,'Cargo_invoked':False,'CLI_probes_only_not_ownerPASS':True},indent=2)+'\n')
