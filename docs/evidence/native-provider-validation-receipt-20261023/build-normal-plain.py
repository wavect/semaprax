from pathlib import Path
import json
ns={};exec(compile(Path('/Volumes/Semaprax/tmp/ci-38383508340/inline-targets-d83/run-focused-root.py').read_text().split('cmd=[')[0],'normal-env','exec'),ns)
o=Path('/Volumes/Semaprax/tmp/ci-38383508340/native-provider-receipt-295');cmd=json.loads((o/'normal-command.json').read_text())['command'];assert not any(x.startswith('-Cincremental') for x in cmd)
ns['g'].guarded(cmd,o/'normal-build-plain.log',ns['env'],start_gib=6.5,pause_gib=3.25,resume_gib=4.5)
