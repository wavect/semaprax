from pathlib import Path
import subprocess,json,hashlib
s=Path('/Volumes/Semaprax/tmp/ci-38383508340');old=s/'native-provider-receipt-295';o=s/'windows-starter-7a252'
subprocess.run(['python3',str(old/'prepare-native-focused.py')],check=True)
r=Path.cwd();rel='src/codegen/native_owned_data_provider/validation_reuse_tests.rs';q=r/'target/ci-private/inline-target-focused-root'/rel;text=q.read_text();first=text.index('\n')+1;text=text[:first]+'#![forbid(unused_imports)]\n'+text[first:];q.write_text(text)
d=json.loads((old/'native-focused-scope.json').read_text());d['target_lint_strengthening']='Only the import-repair module adds forbid(unused_imports), so private registration-related allowances cannot hide its warnings.'
for row in d['all_source_overlays']:
 if row['source']==rel:
  row['overlay_sha256']=hashlib.sha256(q.read_bytes()).hexdigest();row['changes']='Current source bodies unchanged; test registration attributes plus stricter target-module unused-import lint only.'
(o/'native-import-scope.json').write_text(json.dumps(d,indent=2)+'\n')
source=(old/'test-clippy.py').read_text().replace("o/'native-clippy.log'","Path('/Volumes/Semaprax/tmp/ci-38383508340/windows-starter-7a252/native-import-clippy.log')").replace("o/'test-clippy.rmeta'","Path('/Volumes/Semaprax/tmp/ci-38383508340/windows-starter-7a252/native-import-clippy.rmeta')")
exec(compile(source,'actual-target-import-clippy','exec'),{})
