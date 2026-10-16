#!/usr/bin/env python3
"""Finite whole-source/inverse and exact receiver checks; no Rust execution."""
from pathlib import Path
import json,hashlib,subprocess,re
ROOT=Path(__file__).resolve().parents[3];P=Path(__file__).resolve().parent
j=json.loads((P/'source-inverses.json').read_text())
def sha(b):return hashlib.sha256(b).hexdigest()
def git(p):return subprocess.check_output(['git','show',j['base']+':'+p],cwd=ROOT)
for row in j['rows']:
 old=(P/row['before_archive']).read_bytes();new=(ROOT/row['path']).read_bytes()
 assert old==git(row['path']) and sha(old)==row['before_sha256'] and sha(new)==row['after_sha256']
 restored=new.decode()
 for edit in reversed(row['edits']):
  assert restored.count(edit['new'])==1,row['path'];restored=restored.replace(edit['new'],edit['old'],1)
 assert restored.encode()==old,row['path']
for row in json.loads((P/'new-owners.json').read_text()):
 assert sha((ROOT/row['path']).read_bytes())==row['sha256']
 assert len((ROOT/row['path']).read_text().splitlines())<=1500
path='src/cleanup_plan/replay/factored/strategy.rs';old=git(path).decode();new=(ROOT/path).read_text()
def entry(s):return s[s.index('pub(in crate::cleanup_plan::replay) fn validate_authenticated('):s.index('\n/// Add this function')]
original=entry(old);before=(ROOT/'src/cleanup_plan/replay/factored/strategy/before_fitting.rs').read_text()
assert before.endswith(original),'complete old authenticated receiver changed'
restored=new.replace(entry(new),original,1)
for name in ['before_fitting','fitting_tests']:
 registration=f'#[cfg(test)]\n#[path = "strategy/{name}.rs"]\nmod {name};\n'
 assert restored.count(registration)==1;restored=restored.replace(registration,'',1)
assert restored==old,'strategy changes outside entry/registrations'
def tokens(s):
 t=re.findall(r'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z0-9_]*|[0-9]+|[^\s]',s)
 # Targeted rustfmt elides only optional call trailing commas on compact lines.
 return [x for i,x in enumerate(t) if not(x==',' and i+1<len(t) and t[i+1]==')')]
old_tail=original[original.index('let BuiltComparison'):original.index('\n        }\n        Strategy::RefusedArithmeticOverflow')]
new_tail=entry(new)[entry(new).index('let BuiltComparison'):entry(new).rfind('\n}')]
assert tokens(old_tail)==tokens(new_tail),'weighted proof constructor/work/payment tail changed'
for expr in ['let built = build_comparison(program, function, budget, helpers, None)?;', 'let mut candidate = Candidate::from_built(function, built, budget)?;', 'reserve_enumerated(function, budget, upper)?;']:
 assert original.count(expr)==entry(new).count(expr)==1
# Enumeration is absent only from shipping. All old drivers/preflights/builders remain exact.
assert 'validate_typed_control_skeleton_with_index' not in entry(new)
for p in ['src/cleanup_plan/replay/factored.rs','src/cleanup_plan/replay.rs','src/cleanup_plan/replay/lexical_census.rs','src/cleanup_plan/replay/lexical_replay.rs','src/cleanup_plan/replay/lexical_replay/ordinary.rs','src/cleanup_plan/replay/factored/strategy/activation_tests.rs','src/cleanup_plan/replay/factored/strategy/coverage_tests.rs','src/cleanup_plan/replay/factored/strategy/hostile_tests.rs','src/cleanup_plan/replay/factored/strategy/tests.rs']:
 assert (ROOT/p).read_bytes()==git(p),p
for row in j['rows']:assert not row['path'].endswith('.json')
print('PASS: complete five Git before/inverse rows; exact old authenticated entry; unchanged candidate/census/preflight/reservation/dependencies/tests; exact weighted-proof tokens; no second shipping enumeration. Rust/qualification NOT RUN.')
