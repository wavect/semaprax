from pathlib import Path
import json
ns={};exec(compile(Path('/Volumes/Semaprax/tmp/ci-38383508340/inline-targets-d83/run-focused-root.py').read_text().split('cmd=[')[0],'actual-env','exec'),ns)
o=Path('/Volumes/Semaprax/tmp/ci-38383508340/native-provider-receipt-295');cmd=json.loads(Path('/Volumes/Semaprax/tmp/ci-38383508340/native-provider-receipt-295/native-focused-command.json').read_text())['command'];cmd[0]=str(Path(cmd[0]).with_name('clippy-driver'));cmd[cmd.index('-o')+1]=str(o/'test-clippy.rmeta');cmd += ['--emit=metadata','-Dwarnings','-Aunused_macros'];
import tomllib
for group,items in tomllib.loads((Path.cwd()/'Cargo.toml').read_text()).get('lints',{}).items():
 for lint,value in items.items():
  level=value if isinstance(value,str)else value['level'];cmd += ['--'+level,(('clippy::'if group=='clippy'else'')+lint)]
cmd+=['-Aclippy::items_after_test_module']
ns['g'].guarded(cmd,o/'native-clippy.log',ns['env'],start_gib=4.5,pause_gib=2.75,resume_gib=3.5)
