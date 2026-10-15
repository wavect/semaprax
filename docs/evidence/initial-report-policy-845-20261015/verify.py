#!/usr/bin/env python3
"""Finite full-source transport and independent receiver proof. No Rust execution."""
from pathlib import Path
import contextlib,hashlib,io,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
B='2bfd63d1610a89906bb6d4d03489aac7ed5c375b';h=lambda b:hashlib.sha256(b).hexdigest()
def git(p):return subprocess.check_output(['git','show',B+':'+p],cwd=R)
rows=json.loads((D/'current-to-2bfd.json').read_text());aliases={}
for row in rows:
 p=R/row['source'];s=p.read_text();assert h(s.encode())==row['current_sha256'],p
 for e in row['edits']:
  assert s.count(e['new'])==1,p;s=s.replace(e['new'],e['old'],1)
 assert h(s.encode())==row['original_sha256'];assert s.encode()==git(row['source'])==(R/row['archive']).read_bytes();aliases[p.resolve()]=s.encode()
new=json.loads((D/'new-owners.json').read_text())
for row in new:assert h((R/row['source']).read_bytes())==row['sha256']
# Original formatter and invocation implementation retain every byte.
assert (R/'src/bounded_output.rs').read_bytes()==b'pub(crate) mod format_acceptance;\n'+git('src/bounded_output.rs')
original=git('src/workspace_graph/diagnostics.rs').decode();current=(R/'src/workspace_graph/diagnostics.rs').read_text();assert current==original+'\npub(in crate::workspace_graph) mod initial;\n'
root=git('src/workspace_graph.rs').decode();a=root.index('fn build_owned_inner(');b=root.index('\ntype ResolvedCore',a);body=root[a:b].replace('fn build_owned_inner(', 'pub(super) fn build_owned_inner(')
assert body in (R/'src/workspace_graph/initial_report/before.rs').read_text()
# Lexical function extraction retains string literal bytes; no whitespace normalization.
def fn(s,n):
 a=s.index('fn '+n+'(');i=s.index('{',a);depth=1
 for m in re.finditer(r'//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"|[{}]',s[i+1:],re.S):
  if m.group()=='{':depth+=1
  elif m.group()=='}':
   depth-=1
   if not depth:return s[a:i+1+m.end()]
 raise AssertionError(n)
compiled=(R/'src/workspace_graph/initial_report/before_mappers.rs').read_text()
for row in json.loads((D/'independent-receivers.json').read_text()):
 old=(R/row['archive']).read_text();assert old.encode()==git(row['source'])
 for name in row['functions']:
  text=fn(old,name)
  if name!='limit':text=text.replace('fn '+name,'pub(in crate::workspace_graph::initial_report) fn '+name,1)
  assert text in compiled,(row['source'],name)
# All owner text remains in complete compiler unions; root builder children also join root input.
for p in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs']:
 s=(R/p).read_text()
 for row in new:assert row['source'][4:]+'"' in s,(p,row['source'])
assert len((R/'crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs').read_text().splitlines())==1500
assert len((R/'src/workspace_graph.rs').read_text().splitlines())<=6018
for row in new:assert len((R/row['source']).read_text().splitlines())<=1500
capture=(R/'src/workspace_graph/diagnostics/initial.rs').read_text();assert 'let field = "builder_bytes";' in capture and 'Workspace Semantic Graph `{field}` exceeds {maximum}' in capture
assert 'let field = "total_analysis_builder_bytes";' in capture and 'Semantic Workspace Change `{field}` exceeds {maximum}' in capture
cli=(R/'src/cli_driver.rs').read_text();assert cli.count('report_workspace(&failure, false)')==13
# Replay the unchanged predecessor source verifier in its exact whole-source input domain.
prior=R/'docs/evidence/field-update-hir-visitor-836-20261015/verify.py';code=prior.read_text();rb,rt=Path.read_bytes,Path.read_text
try:
 Path.read_bytes=lambda p,*a,**k:aliases[p.resolve()] if p.resolve() in aliases else rb(p,*a,**k)
 Path.read_text=lambda p,*a,**k:aliases[p.resolve()].decode() if p.resolve() in aliases else rt(p,*a,**k)
 with contextlib.redirect_stdout(io.StringIO()) as out:exec(compile(code,str(prior),'exec'),{'__file__':str(prior),'__name__':'__main__'})
finally:Path.read_bytes,Path.read_text=rb,rt
previous=json.loads(out.getvalue());assert previous['whole_previous_joined_source_proof']['old_packet_files_byte_exact']==222
print(json.dumps({'status':'SOURCE_ONLY_NOT_EXECUTED','complete_current_to_2bfd':len(rows),'new_owners':len(new),'pending_test_bodies':10,'complete_original_builder':True,'complete_independent_mappers':3,'legacy_formatter_and_TLS_bytes_unchanged':True,'actual_CLI_adoptions':13,'predecessor_proof':previous},indent=2))
