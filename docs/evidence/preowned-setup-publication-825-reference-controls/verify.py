#!/usr/bin/env python3
"""Whole-source correction and original-packet replay; no Rust execution."""
from pathlib import Path
import hashlib,json,subprocess
D=Path(__file__).resolve().parent;R=D.parents[2]
P=json.loads((D/'source-inverses.json').read_text());h=lambda b:hashlib.sha256(b).hexdigest()
assert len(P['sources'])==1
row=P['sources'][0];old=row['before'].encode();current=(R/row['path']).read_bytes()
assert old==subprocess.check_output(['git','show',P['base']+':'+row['path']])
assert old==(D/'attempt.rs.before.txt').read_bytes()
assert current==row['current'].encode()
assert h(old)==row['before_sha256'] and h(current)==row['current_sha256']
addition='''        // Separate outer match receivers are real controls, in addition to
        // the inner &mut CoreFailure argument paid by the report owner.
        .and_then(|bytes| bytes.checked_add(size_of::<&Result<T, CoreFailure>>()))
        .and_then(|bytes| bytes.checked_add(size_of::<&mut Result<T, CoreFailure>>()))
        .and_then(|bytes| bytes.checked_add(size_of::<&Setup>()))
        // The copied *setup initializer and seal_setup's by-value receiver.
        .and_then(|bytes| bytes.checked_add(2 * size_of::<Setup>()))
'''
assert current.decode().count(addition)==1
assert current.decode().replace(addition,'')==old.decode()
assert current.decode().index(addition)<current.decode().index('let owner = match')
# Replay every original complete source binding in its named prior view.
prior=R/'docs/evidence/preowned-setup-publication-825'
for p in prior.iterdir():
 if p.is_file():assert p.read_bytes()==subprocess.check_output(['git','show',P['base']+':'+str(p.relative_to(R))])
original=json.loads((prior/'source-inverses.json').read_text())
for r in original['sources']:
 b=old if r['path']==row['path'] else (R/r['path']).read_bytes()
 assert b==r['current'].encode() and h(b)==r['current_sha256']
for r in original['additions']:
 assert (R/r['path']).read_bytes()==r['source'].encode()
assert len(current.decode().splitlines())<=1500
print(json.dumps(dict(status='SOURCE_ONLY_PASS',whole_current_inverse=True,original_packet_unchanged=True,original_complete_bindings_replayed=True,new_layouts=['&Result<T,CoreFailure>','&mut Result<T,CoreFailure>','&Setup','Setup copied initializer','Setup parameter receiver'],rust_execution=False)))
