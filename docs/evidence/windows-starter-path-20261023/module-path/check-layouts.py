from pathlib import Path
from importlib.machinery import SourceFileLoader
import os,json,subprocess,hashlib,shutil
r=Path.cwd();s=Path('/Volumes/Semaprax/tmp/ci-38383508340');o=s/'module-path-536';m=r/'target/ci-private/starter-module-layout';(m/'src/new_project/application').mkdir(parents=True,exist_ok=True);(m/'tests').mkdir(exist_ok=True)
ns={};exec(compile((s/'windows-starter-7a252/check-reader.py').read_text().split("p=r/")[0],'existing-original-source-parser','exec'),ns);function=ns['function'];g=ns['g'];t=ns['t']
app=r/'crates/semaprax-toolchain/src/new_project/application.rs';current=app.read_text();before=subprocess.check_output(['git','show','53664cbe079f1ae93d3c105f54052efded7ffde1:'+str(app.relative_to(r))],text=True);parent=(app.parent/'../new_project.rs').resolve().read_text();a=parent.rfind('#[derive',0,parent.index('pub(crate) struct NewProjectFailure'));b=parent.index('\npub(crate) trait WriteHook',a);failure=parent[a:b]
header='use std::{fmt,fs,path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering}};\n'+next(x for x in parent.splitlines()if x.startswith('static STAGING_SERIAL:'))+'\n'+failure+'\n'
registration=parent[:parent.index('\n#[path',1)];assert registration=='#[path = "new_project/application.rs"]\nmod application;';(m/'src/new_project.rs').write_text(registration+'\n'+header)
(m/'src/main.rs').write_text('#![allow(dead_code)]\nmod new_project;\n')
integration=(r/'crates/semaprax-toolchain/tests/cli_new_project_v1.rs').read_text();registration='#[path = "../src/new_project.rs"]\nmod new_project;';assert registration in integration;(m/'tests/layout.rs').write_text('#![allow(dead_code)]\n'+registration+'\n')
shutil.copyfile(app.with_suffix('')/'held_input_path.rs',m/'src/new_project/application/held_input_path.rs')
platform=json.loads((s/'windows-starter-7a252/reader-scope.json').read_text())['genuine_platform'];normal=json.loads((s/'native-provider-receipt-295/normal-command.json').read_text())['command'];deps=next(x for x in normal if x.startswith('dependency='));env=dict(os.environ,RUSTC_BOOTSTRAP='1',CARGO_MANIFEST_DIR=str(r));rows=[]
def subject(text):
 prefix=text[:text.index('pub(super) fn run(')].replace(function(text,'diagnostic'),'')
 return prefix+'\n'+function(text,'root')+'\n#[test]\n'+function(text,'starter_input_held_regular_reader_preserves_exact_bytes_and_bounds')+'\n#[test]\n'+function(text,'starter_reader_refuses_parent_rebinding_after_the_held_open')+'\n'
for state,text in [('before',before),('current',current)]:
 (m/'src/new_project/application.rs').write_text(subject(text))
 for name,source in [('production',m/'src/main.rs'),('integration',m/'tests/layout.rs')]:
  output=o/(state+'-'+name);cmd=[str(t/'rustc'),'--edition=2021','--test','-Cdebuginfo=0','-Ccodegen-units=16','-L',deps,'-L','dependency='+str(r/'target/ci-private/source-gate-release-1.97/release/deps'),'--extern','semaprax_native_rust_interop_platform='+platform,str(source),'-o',str(output)]
  try:g.guarded(cmd,o/(state+'-'+name+'-build.log'),env,start_gib=3.0,pause_gib=2.0,resume_gib=3.0)
  except SystemExit as error:
   assert state=='before'and error.code==1
   diagnostic=(o/(state+'-'+name+'-build.log')).read_text();assert 'E0583'in diagnostic and 'held_input_path'in diagnostic
   rows.append(dict(state=state,layout=name,command=cmd,result='EXPECTED_MODULE_DISCOVERY_REFUSAL'));continue
  assert state=='current','predecessor must reproduce missing module'
  g.guarded([str(output),'--test-threads=1'],o/(name+'-tests.log'),env,start_gib=3.0,pause_gib=2.0,resume_gib=3.0)
  clippy=[str(t/'clippy-driver'),*cmd[1:],'--emit=metadata','-Dwarnings'];clippy[clippy.index('-o')+1]=str(output)+'.rmeta';g.guarded(clippy,o/(name+'-clippy.log'),env,start_gib=3.0,pause_gib=2.0,resume_gib=3.0)
  rows.append(dict(state=state,layout=name,command=cmd,result='SEVEN_GENUINE_READER_AND_PREFIX_TESTS_PASS'))
(o/'layout-verification.json').write_text(json.dumps(dict(base_head='53664cbe079f1ae93d3c105f54052efded7ffde1',source_sha256=hashlib.sha256(app.read_bytes()).hexdigest(),actual_parent_and_integration_module_paths_preserved=True,no_child_module_path_rebasing=True,actual_complete_reader_functions_and_failure_type=True,no_mock_platform_or_IR=True,scope='same seven controls in production and integration module layouts; genuine native platform; no hosted Windows or full application qualification',rows=rows),indent=2)+'\n');print('Both original module contexts reproduce before and pass current.')
