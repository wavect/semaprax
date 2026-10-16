#!/usr/bin/env python3
"""Finite source-only owner/inverse check; no Cargo or binaries."""
import hashlib,json,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[3];P=Path(__file__).resolve().parent
sha=lambda b:hashlib.sha256(b).hexdigest()
table=json.loads((P/'current-to-55e.json').read_text());row=table['rows'][0]
old=(P/row['before_archive']).read_bytes();current=(R/row['path']).read_bytes()
assert sha(old)==row['before_sha256'] and sha(current)==row['current_sha256']
assert old==subprocess.check_output(['git','show',table['base']+':'+row['path']],cwd=R)
restored=current.decode()
for edit in row['inverse']:
 assert restored.count(edit['old'])==1
 restored=restored.replace(edit['old'],edit['new'])
assert restored.encode()==old and current.startswith(old)
changed=subprocess.check_output(['git','diff',table['base'],'--name-only','--diff-filter=M','--','src','tests','crates'],cwd=R,text=True).splitlines()
assert changed==[row['path']],changed
# Exact installed V2 parser definitions, applied only to this existing child.
parser=Path('/Volumes/Semaprax/provenance/qualification-registration-module-path-20261014.py')
defs=parser.read_text().split("parse(R/'src/lib.rs'",1)[0];ns={};exec(compile(defs,str(parser),'exec'),ns);ns['R']=R
prefix=['cleanup_plan','replay','factored','oracle_order','tests']
ns['parse'](R/row['path'],prefix,[],'lib','semaprax')
selector=json.loads((P/'pending-selectors.json').read_text())['checks'][0]['selector']
key=('semaprax','lib',selector);assert key in ns['IDX'] and not ns['IDX'][key]['ignored']
assert len(ns['IDX'])==4 # three original literal owners plus one additive owner
proof={'status':'SOURCE_ONLY_PASS_UNEXECUTED','base':table['base'],'complete_before_git_archive_inverse':True,'only_existing_test_child_changed':True,'old_all_bytes_prefix_retained':True,'new_literal_owner':selector,'source_path':row['path'],'before_sha256':sha(old),'current_sha256':sha(current),'literal_child_functions':4,'source_lines':len(current.splitlines()),'parser_sha256':sha(parser.read_bytes()),'global_source_front_and_authority':'Deferred to parent later pin/test seal; no protected table rewritten','Cargo_or_tests':False}
(P/'source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof))
