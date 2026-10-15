from pathlib import Path
import difflib,hashlib,json,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;H='4f5b5ccfad20ce100205d6cf666c1b6c27330818';g=lambda *a:subprocess.check_output(['git',*a],cwd=R);h=lambda b:hashlib.sha256(b).hexdigest();tracked=set(g('ls-tree','-r','--name-only',H).decode().splitlines());rows=[]
for source in g('diff','--name-only',H).decode().splitlines():
 if source not in tracked:continue
 old=g('show',H+':'+source);new=(R/source).read_bytes()
 if old==new:continue
 archive=D/'before'/source;archive.parent.mkdir(parents=True,exist_ok=True);archive.write_bytes(old);a=new.decode().splitlines(True);b=old.decode().splitlines(True);edits=[]
 for context in range(2,33):
  candidate=[{'new':''.join(a[x[0][1]:x[-1][2]]),'old':''.join(b[x[0][3]:x[-1][4]])} for x in difflib.SequenceMatcher(None,a,b,autojunk=False).get_grouped_opcodes(context)];restored=new
  for e in candidate:
   if not e['new'] or restored.count(e['new'].encode())!=1:break
   restored=restored.replace(e['new'].encode(),e['old'].encode(),1)
  else:
   if restored==old:edits=candidate;break
 assert edits
 rows.append({'source':source,'checkpoint_revision':H,'archive':str(archive.relative_to(R)),'current_sha256':h(new),'original_sha256':h(old),'edits':edits})
assert len(rows)==7,[r['source'] for r in rows]
(D/'current-to-4f5b.json').write_text(json.dumps(rows,indent=2)+'\n');(D/'source-inverses.json').write_bytes((D/'current-to-4f5b.json').read_bytes());print('7wholebeforeinputs; newcompiledJSONbytes',(D/'current-to-4f5b.json').stat().st_size)
