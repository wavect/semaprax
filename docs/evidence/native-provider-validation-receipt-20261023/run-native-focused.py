from pathlib import Path
import json
ns={}
source=Path('/Volumes/Semaprax/tmp/ci-38383508340/inline-targets-d83/run-focused-root.py').read_text().split('cmd=[')[0]
exec(compile(source,'original-focused-environment','exec'),ns)
o=Path('/Volumes/Semaprax/tmp/ci-38383508340/native-provider-receipt-295');m=ns['m'];env=ns['env'];env['RUST_BACKTRACE']='1';g=ns['g']
cmd=json.loads(Path('/Volumes/Semaprax/tmp/ci-38383508340/inline-targets-d83/native-focused-command.json').read_text())['command']
cmd=[x.replace('-Copt-level=1','-Copt-level=0').replace('-Cmetadata=native_nine_inline_targets_d83','-Cmetadata=native_twentyfour_implicit_error_code_437b') for x in cmd]
cmd=[x.replace('-Ccodegen-units=64','-Ccodegen-units=256') for x in cmd]
cmd+=['-Cdebug-assertions=on','-Coverflow-checks=on']
(o/'native-focused-command.json').write_text(json.dumps(dict(command=cmd,scope='actual complete current cfg(test) compiler; selected tests registered and unrelated test registrations disabled; four external registrations are filtered, never claimed executed'),indent=2))
g.guarded(cmd,o/'native-focused-build.log',env,start_gib=6.5,pause_gib=3.0,resume_gib=4.0)
for selector,name in [('codegen::native_owned_data_provider::validation_reuse_tests::','provider-validation-controls.log'),('codegen::native_emit::emission::validation_reuse_tests::','native-validation-controls.log'),('call_index::validation_receipt_tests::','call-index-validation-controls.log'),('codegen::native_resource::tests::','native-resource-controls.log'),('codegen::native_emit::owned_strings::tests::','owned-string-controls.log')]:
 g.guarded([cmd[cmd.index('-o')+1],selector,'--test-threads=1'],o/name,env)
