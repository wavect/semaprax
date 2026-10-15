from pathlib import Path
import hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='56dc66694';h=lambda b:hashlib.sha256(b).hexdigest();rows=[]
for i,p in enumerate(['src/cleanup_plan/replay/selected_context/child/tests/source/compressed_fixture.rs','src/cleanup_plan/replay/selected_context/child/tests/source.rs','src/source_reflows_merge.rs']):
 old=subprocess.check_output(['git','show',BASE+':'+p],cwd=R);new=(R/p).read_bytes();a=D/f'before-{i}.txt';a.write_bytes(old);rows.append({'source':p,'checkpoint_revision':subprocess.check_output(['git','rev-parse',BASE],cwd=R,text=True).strip(),'archive':str(a.relative_to(R)),'current_sha256':h(new),'original_sha256':h(old),'identity_binding':new==old,'edits':[{'new':new.decode(),'old':old.decode()}]})
(D/'current-to-56dc.json').write_text(json.dumps(rows,indent=2)+'\n');(D/'source-inverses.json').write_text(json.dumps([r for r in rows if not r['identity_binding']],indent=2)+'\n')
