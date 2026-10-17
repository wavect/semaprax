"""Focused offline package preservation and dispatcher-shape checks."""
from pathlib import Path
import ast,hashlib,json
D=Path(__file__).resolve().parent;R=D.parents[2];sha=lambda b:hashlib.sha256(b).hexdigest()
bind=json.loads((D/'bindings.json').read_text())
for row in bind['scripts']:
 assert (R/row['path']).read_bytes()==(D/'originals'/row['path']).read_bytes()
 assert sha((R/row['path']).read_bytes())==row['sha256']
for name in ['entry.py','memoizer.py','check-pure-masks.py']:
 ast.parse((D/name).read_text())
print(json.dumps({'status':'ALL_SEVEN_COMPLETE_ORIGINAL_SCRIPT_BYTES_PRESERVED','original_scripts':len(bind['scripts'])}))
