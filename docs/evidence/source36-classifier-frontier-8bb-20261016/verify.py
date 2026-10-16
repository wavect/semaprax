"""Finite actual-failure fixes; final outer namespace/role binding remains pending."""
from pathlib import Path
import hashlib,json,re,subprocess
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent;BASE='4fba2ee58ab595eb6ee1dc9aaa4bfa975358a7a4';h=lambda s:hashlib.sha256(s.encode()).hexdigest()
for folder in [D,R/'docs/evidence/projection-facade-frontier-8bb-20261016']:
 for row in json.loads((folder/'source-inverses.json').read_text()):
  p=row['source'];current=(R/p).read_text();old=subprocess.check_output(['git','show',BASE+':'+p],cwd=R).decode();assert h(current)==row['current_sha256'] and h(old)==row['original_sha256'];assert (R/row['archive']).read_text()==old
  for e in row['edits']:assert current.count(e['new'])==1;current=current.replace(e['new'],e['old'],1)
  assert current==old
source='src/bounded_output/classifier_string.rs';current=(R/source).read_text()
for f in ['next-main-beb0-847-joined-20261016/current-to-8bb.json','private-175-actual-fixes-20261015/current-to-c159.json','ordered-amortized-root-domain-20261015/current-to-35cc.json','storage-oracle-compiler-fix-join-20261015/current-to-d719.json','sourceguard-input-domains-7a87/classifier-input.json']:
 rows=[r for r in json.loads((R/'docs/evidence'/f).read_text()) if r['source']==source];assert len(rows)<=1
 for row in rows:
  assert h(current)==row['current_sha256']
  for e in row['edits']:assert current.count(e['new'])==1;current=current.replace(e['new'],e['old'],1)
  assert h(current)==row['original_sha256']
assert h(current)=='ee3073593ac4c8ab657dc5daa7500108b60c4c64bc898e877208f60a10d56a34'
F=R/'docs/evidence/projection-facade-frontier-8bb-20261016';s=(R/'tests/workspace/semantic_graph/source_inventory.rs').read_text();block=s[s.index('macro_rules! semantic_graph_projection_root_source_inventory'):s.index('macro_rules! semantic_graph_projection_child_source_inventory')];parts=[]
for p,literal in re.findall(r'include_str!\(\s*"([^"]+)"\s*\)|((?:"(?:\\.|[^"\\])*"))',block):parts.append(json.loads(literal) if literal else (R/'tests/workspace'/p).read_text())
current=''.join(parts);row=json.loads((F/'root-input.json').read_text());assert h(current)==row['current_sha256'] and current==(F/'current-root.txt').read_text()
for e in row['edits']:assert current.count(e['new'])==1;current=current.replace(e['new'],e['old'],1)
assert h(current)==row['original_sha256'] and current==(F/'original-root.txt').read_text();legacy=json.loads((R/'docs/evidence/ae91-source-consumer-corrections-20261015/facade.json').read_text());assert current==legacy['edits'][0]['new'] and h(current)==legacy['current_sha256']
print('SOURCE_ONLY_PASS: complete classifier and virtual-root domains; both full immediate source inverses. Global outer stage and authority rebind pending.')
