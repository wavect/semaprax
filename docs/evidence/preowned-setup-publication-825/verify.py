#!/usr/bin/env python3
"""Whole-source checks only. This is not Rust compilation or test execution."""
from pathlib import Path
import hashlib,json,subprocess
D=Path(__file__).resolve().parent
R=D.parents[2]
P=json.loads((D/'source-inverses.json').read_text())
h=lambda x:hashlib.sha256(x).hexdigest()
for row in P['sources']:
 old=subprocess.check_output(['git','show',P['base']+':'+row['path']])
 current=(R/row['path']).read_bytes()
 assert old==row['before'].encode() and h(old)==row['before_sha256'],row['path']
 assert current==row['current'].encode() and h(current)==row['current_sha256'],row['path']
for row in P['additions']:
 current=(R/row['path']).read_bytes()
 assert current==row['source'].encode() and h(current)==row['sha256'],row['path']
C=R/'src/workspace_graph/constructor_failure'
old_report=(C/'before_publication/report.rs').read_text()
for path in ['stages.rs','tests.rs']:
 old_report=old_report.replace('#[path = "../report/'+path+'"]\n','')
assert old_report==(D/'report.rs.before.txt').read_text()
assert (C/'before_publication/attempt.rs').read_bytes()==(D/'attempt.rs.before.txt').read_bytes()
for name,decl,end in [
 ('receiving.rs','    pub(in crate::workspace_graph) fn into_legacy_unfunded(self) -> Vec<Diagnostic> {','\n}\nimpl Receiver {'),
 ('constructor_failure.rs','    pub(super) fn into_legacy_unfunded(self) -> Vec<Diagnostic> {','\n}\nimpl From<Vec<Diagnostic>> for Failure {')]:
 current=(C/name if name!='constructor_failure.rs' else C.parent/name).read_text()
 old=(D/(name+'.before.txt')).read_text()
 assert current[current.index(decl):current.index(end)]==old[old.index(decl):old.index(end)]
# The independently compiled old attempt shares the unchanged enum representation,
# never the new report owner or the new transfer method.
current=(C/'receiving.rs').read_text();old=(D/'receiving.rs.before.txt').read_text()
start='pub(in crate::workspace_graph) enum CoreFailure'
assert current[current.index(start):current.index('impl CoreFailure')]==old[old.index(start):old.index('impl CoreFailure')]
union=(R/'tests/workspace/semantic_graph/source_inventory.rs').read_text()
for row in P['additions']:
 assert union.count(row['path']+'"')==1,row['path']
for row in P['sources']+P['additions']:
 if row['path'].endswith('.rs'):assert len((R/row['path']).read_text().splitlines())<=1500
workspace='src/workspace_graph.rs'
assert (R/workspace).read_bytes()==subprocess.check_output(['git','show',P['base']+':'+workspace])
assert not any(row['path'].startswith('docs/evidence/') for row in P['sources'])
print(json.dumps(dict(status='SOURCE_ONLY_PASS',changed_complete_sources=len(P['sources']),complete_added_sources=len(P['additions']),rust_execution=False,issue825='OPEN_PARTIAL')))
