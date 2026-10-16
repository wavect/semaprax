#!/opt/homebrew/bin/python3
"""Source-only whole-row partition controls. Never regenerates common authority."""
from pathlib import Path
import argparse,ast,copy,hashlib,json,runpy,subprocess,tempfile
R=Path(__file__).resolve().parents[3];p=argparse.ArgumentParser();p.add_argument('--logical-table',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output.resolve();assert out.is_relative_to('/Volumes/Semaprax');out.mkdir(parents=True,exist_ok=False)
H=runpy.run_path(str(R/'docs/evidence/joined-batch-custody-20261016/table-parts.py'));raw=a.logical_table.read_bytes();rows=json.loads(raw);domain=dict(name='Previous16c',table='current-to-Previous16c.json',keys=[r['source']for r in rows]);refused=[]
def bad(name,action):
    try:action()
    except (AssertionError,FileNotFoundError,ValueError,TypeError,KeyError):refused.append(name)
    else:raise AssertionError('hostile parts accepted: '+name)
with tempfile.TemporaryDirectory(dir=out) as directory:
    root=Path(directory);H['write_parts'](root,domain,rows);saved={p['table']:(root/p['table']).read_bytes()for p in domain['parts']}
    assert H['load_rows'](root,domain)==rows
    assert H['encoded'](H['load_rows'](root,domain))==raw
    assert len(domain['parts'])>=2
    for index,group in enumerate(H['split'](rows)):
        for row in group:
            selected=max(i for i,part in enumerate(domain['parts'])if row['source']>=part['first'])
            assert selected==index
    for name,edit in [
        ('gap-index',lambda d:d['parts'][1].update(table='current-to-Previous16c-part-002.json')),
        ('reordered-parts',lambda d:d['parts'].reverse()),
        ('duplicate-part',lambda d:d['parts'].insert(1,copy.deepcopy(d['parts'][0]))),
        ('missing-part',lambda d:d['parts'].pop()),
        ('forged-sha',lambda d:d['parts'][0].update(sha256='00'*32)),
        ('forged-length',lambda d:d['parts'][0].update(length=d['parts'][0]['length']-1)),
        ('forged-boundary',lambda d:d['parts'][1].update(first='foreign')),
        ('foreign-path',lambda d:d['parts'][0].update(table='../foreign.json')),
        ('foreign-domain',lambda d:d.update(name='Foreign')),
    ]:
        d=copy.deepcopy(domain);edit(d);bad(name,lambda:H['load_rows'](root,d))
    for name,edit in [('missing-row',lambda g:g.pop()),('duplicate-row',lambda g:g.insert(0,copy.deepcopy(g[0]))),('foreign-row',lambda g:g[0].update(source='foreign/source.rs'))]:
        d=copy.deepcopy(domain);part=d['parts'][1];group=json.loads(saved[part['table']]);edit(group);payload=H['encoded'](group);(root/part['table']).write_bytes(payload)
        # Even re-signing the forged part cannot defeat whole ordered key coverage.
        part.update(length=len(payload),sha256=hashlib.sha256(payload).hexdigest(),rows=len(group),first=group[0]['source'],last=group[-1]['source'])
        bad(name,lambda:H['load_rows'](root,d));(root/part['table']).write_bytes(saved[part['table']])
    target=root/domain['parts'][1]['table'];target.unlink();bad('missing-file',lambda:H['load_rows'](root,domain));target.write_bytes(saved[target.name])
    target.write_bytes(target.read_bytes()+b' ');bad('trailing-byte',lambda:H['load_rows'](root,domain));target.write_bytes(saved[target.name])
    row={'source':'a','payload':''};row['payload']='x'*(H['CAP']-len(H['encoded']([row])));assert len(H['encoded']([row]))==H['CAP'];assert len(H['split']([row,{'source':'b'}]))==2
    row['payload']+='x';bad('one-row-over-cap',lambda:H['split']([row]))
    proof=dict(logical_table_bytes=len(raw),logical_rows=len(rows),logical_sha256=hashlib.sha256(raw).hexdigest(),parts=domain['parts'],whole_rows_and_logical_bytes_exact=True,all_source_keys_route_to_their_exact_part=True,exact_cap_boundary=True,hostile_refusals=refused)
# Existing reader/cap and every original test remain complete, byte-exact prefixes.
def original(path):return subprocess.check_output(['git','show','88cc7352d:'+path],cwd=R)
old=original('src/source_reflows_merge/runtime_json.rs');current=(R/'src/source_reflows_merge/runtime_json.rs').read_bytes();start=b'fn read_fixture(';end=b'#[path = "runtime_json/tests.rs"]';assert old.split(start,1)[1].split(end,1)[0]==current.split(start,1)[1].split(end,1)[0]
assert (R/'src/source_reflows_merge/runtime_json/tests.rs').read_bytes().startswith(original('src/source_reflows_merge/runtime_json/tests.rs'))
protected=[]
for name in ['joined-refresh-receipt.json','postpin-refresh-receipt.json','generate-authority.py','verify-source.py','verify-authority.py','prepare-arm-overlay.py']:
    path='docs/evidence/a3-e47-final-custody-20261016/'+name;data=(R/path).read_bytes();assert data==subprocess.check_output(['git','show','88cc7352d:'+path],cwd=R);protected.append(dict(path=path,sha256=hashlib.sha256(data).hexdigest()))
for path in [R/'docs/evidence/joined-batch-custody-20261016/table-parts.py',R/'docs/evidence/joined-batch-custody-20261016/verify-source-parts.py',R/'docs/evidence/joined-batch-custody-20261016/refresh.py']:ast.parse(path.read_bytes(),filename=str(path))
proof.update(status='PASS_SOURCE_ONLY_BOUNDED_PARTS',unchanged_reader_cap_and_all_old_rust_tests=True,all_six_protected_originals=protected,common_refresh=False,rust_executed=False)
(out/'receipt.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
