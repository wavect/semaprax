"""Exact NEWMAIN raw acquisition before the complete unchanged043 view."""
from pathlib import Path
import hashlib,runpy
ROOT=Path(__file__).resolve().parents[3]
SOURCE='src/source_reflows/semantic_receivers.rs'
CURRENT_SHA='3697d98f51d3d633a280ffd7ee0656c0d45e660e061cee5275ee81189d1ee1a4'
CURRENT_LEN=30256
CURRENT_ARCHIVE='docs/evidence/main043-newmain-acquisition-20261017/current-inputs/3697d98f51d3d633a280ffd7ee0656c0d45e660e061cee5275ee81189d1ee1a4.txt'
OLD_SHA='d19607ffcbca7e4d676a774eb2f21d81293430f6c5985a7afd6409d9bb7589df'
OLD_SOURCE='docs/evidence/main34d-three-source-domains-20261017/original-programs/src__source_reflows__semantic_receivers.rs.ours492.txt'
OLD_VIEW='docs/evidence/main043-independent-custody-20261016/semantic-receiver-view.py'
PINS={'docs/evidence/main043-independent-custody-20261016/semantic-receiver-view.py': '79b1f3f8fd8ed42ad8baa5552f468ad7b884be1e51272dcf8e5a94fc5e373823', 'docs/evidence/main043-independent-custody-20261016/semantic-receiver-outer.py': '5ee9c8fc3272a4110bf2516b263d37697f98ea301190989645b02f0333983db4', 'docs/evidence/main043-semantic-acquisition-20261017/current-to-043-stage.json': '63bd9f8a4f8d19369a78798a41278e5b96ab0732361598656884efd6af159ff9', 'docs/evidence/main043-semantic-acquisition-20261017/current.rs.txt': 'd19607ffcbca7e4d676a774eb2f21d81293430f6c5985a7afd6409d9bb7589df', 'docs/evidence/main043-semantic-acquisition-20261017/prior-stage.rs.txt': '457d115f41607edd272dbf214714a0df276dfa75fb8a86a2cb313586c5bc495c', 'docs/evidence/main043-independent-custody-20261016/semantic-receiver-historical-view/current-to-main043.json': '1aa0092ae0b2503a0fc398efe2dd3454f9940d052f6c3e9cf5861c57e9d05b5d', 'docs/evidence/main043-independent-custody-20261016/semantic-receiver-historical-view/main043-semantic_receivers.rs.txt': '651d174d04fad97da5c08ad215a869fd395a7d691d4a78dc901a99e814fc8252', 'docs/evidence/main34d-three-source-domains-20261017/original-programs/src__source_reflows__semantic_receivers.rs.ours492.txt': 'd19607ffcbca7e4d676a774eb2f21d81293430f6c5985a7afd6409d9bb7589df', 'docs/evidence/main043-newmain-acquisition-20261017/current.rs.txt': '6816d73e025e149033a83a0bce3f0f1bd3072563fad3a8b715395274a6a8eb2b', 'docs/evidence/main043-newmain-acquisition-20261017/current-inputs/ba2ddbf8486039ba24fb0f36a90dea66910e931cbb71115b4d2eab114906430b.txt': 'ba2ddbf8486039ba24fb0f36a90dea66910e931cbb71115b4d2eab114906430b', 'docs/evidence/main043-newmain-acquisition-20261017/current-inputs/3697d98f51d3d633a280ffd7ee0656c0d45e660e061cee5275ee81189d1ee1a4.txt': '3697d98f51d3d633a280ffd7ee0656c0d45e660e061cee5275ee81189d1ee1a4'}
BEFORE="_acquire=runpy.run_path(str(_outer))['project']"
AFTER="_main34d_old_acquire=runpy.run_path(str(_outer))['project']\n_acquire=lambda current: _main34d_old_acquire(_main34d_acquire(_main34d_repo,current))"
sha=lambda b:hashlib.sha256(b).hexdigest()
def authenticated(repo):
 repo=Path(repo).resolve();assert repo==ROOT
 assert b'\r' not in Path(__file__).read_bytes(),'wrapper physical CRLF drift'
 for path,expected in PINS.items():assert sha((repo/path).read_bytes())==expected,('complete pinned historical input drift',path)
 return repo
def acquire(repo,current):
 repo=authenticated(repo)
 raw=(repo/SOURCE).read_bytes()
 assert type(current) is str and raw.decode('utf8')==current and current.encode('utf8')==raw,'physical raw source differs from supplied UTF8 input'
 assert len(raw)==CURRENT_LEN and sha(raw)==CURRENT_SHA
 assert raw==(repo/CURRENT_ARCHIVE).read_bytes()
 previous=(repo/OLD_SOURCE).read_bytes()
 assert sha(previous)==OLD_SHA and len(previous)==12626
 return previous.decode('utf8')
def historical(repo,current):
 repo=authenticated(repo)
 # Physical bytes and exact input are checked before executing the old view.
 acquire(repo,current)
 original=(repo/OLD_VIEW).read_bytes().decode('utf8')
 assert original.count(BEFORE)==1
 adapted=original.replace(BEFORE,AFTER,1)
 assert adapted.count(AFTER)==1 and adapted.replace(AFTER,BEFORE,1)==original
 namespace={'__file__':str(repo/OLD_VIEW),'__name__':'_historical_main34d_closed_input','_main34d_acquire':acquire,'_main34d_repo':repo}
 exec(compile(adapted,str(repo/OLD_VIEW),'exec'),namespace)
 return namespace['historical'](repo,current)
