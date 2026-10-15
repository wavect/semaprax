#!/usr/bin/env python3
"""Complete source/inventory proof only; does not execute Rust or fixtures."""
from pathlib import Path
import json,subprocess,hashlib
D=Path(__file__).resolve().parent;R=D.parents[2];P=json.loads((D/'source-inverses.json').read_text());h=lambda b:hashlib.sha256(b).hexdigest()
assert len(P['sources'])==1
row=P['sources'][0];old=row['before'].encode();current=(R/row['path']).read_bytes()
assert old==subprocess.check_output(['git','show',P['base']+':'+row['path']])==(D/'application_starter.rs.before.txt').read_bytes()
assert h(old)==row['before_sha256'] and h(current)==row['current_sha256'] and current==row['current'].encode()
s=current.decode();start=s.index('    let raw_before = read_tree(&source);');end=s.index('    let before = read_tree(&source);',start)
assert s[:start]+s[end:]==old.decode(),'entire original source restored, not assertion selection'
added=s[start:end]
for required in ['assert!(!noncanonical.status.success());','contains("SPX-G170")','assert!(!raw_output.exists());','assert_eq!(read_tree(&source), raw_before);','for (relative, bytes) in &raw_before','relative.ends_with(".spx")','semaprax::parse(text, relative)','semaprax::format::canonical(&parsed)']:
 assert required in added,required
V=json.loads((D/'raw-fixture-inventory.json').read_text())
actual=subprocess.check_output(['git','ls-tree','-r','--name-only',P['base'],'--','examples/shiftsim-typed-record-successor'],text=True).splitlines()
assert actual==[r['path'] for r in V['sources']]
for r in V['sources']:
 b=(R/r['path']).read_bytes();assert len(b)==r['bytes'] and h(b)==r['sha256'];assert b==subprocess.check_output(['git','show',P['base']+':'+r['path']])
assert len(s.splitlines())<=1500
print(json.dumps(dict(status='SOURCE_ONLY_PASS',complete_test_inverse=True,entire_original_source_restored=True,full_raw_fixture_files=len(actual),rust_execution=False,issue837='OPEN')))
