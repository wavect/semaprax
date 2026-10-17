"""Whole original canonical source controls, with finite PaidMany acquisition."""
from pathlib import Path
import hashlib,json
P=Path(__file__).resolve();R=P.parents[3];N=R/'docs/evidence/paid-many-canonical-ingress-20261017'
before=(N/'check-source.before.py.txt').read_bytes()
assert len(before)==6063 and hashlib.sha256(before).hexdigest()=='85055bcfa1ef269afa779835e5e6b399af772b283a7a29bbf5777c9f6fd80ae9'
edits=(N/'checker-edits.json').read_bytes()
assert len(edits)==780 and hashlib.sha256(edits).hexdigest()=='c3afed2626ffcfc6fb214cfb64ff4bb50ddf5ced7d7cf3c6f467f44ca1d99c4a'
text=before.decode()
for old,new in json.loads(edits):assert text.count(old)==1;text=text.replace(old,new,1)
restored=text
for old,new in reversed(json.loads(edits)):assert restored.count(new)==1;restored=restored.replace(new,old,1)
assert restored.encode()==before
exec(compile(text,str(P),'exec'),{'__file__':str(P)})
