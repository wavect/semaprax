from pathlib import Path
import hashlib,sys,os,stat
R=Path(__file__).resolve().parents[3];D=Path(__file__).resolve().parent
def bounded(path,length,digest):
 assert 0<=length<=262144 and hasattr(os,'O_NOFOLLOW')
 fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 identity=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  first=os.fstat(fd);assert stat.S_ISREG(first.st_mode) and first.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as stream:
   raw=stream.read(length);assert len(raw)==length and not stream.read(1)
  after=os.fstat(fd);physical=os.lstat(path)
  assert stat.S_ISREG(physical.st_mode) and identity(first)==identity(after)==identity(physical)
  assert hashlib.sha256(raw).hexdigest()==digest
  raw.decode('utf8');return raw
 finally:os.close(fd)
P=R/'docs/evidence/ordinary-plan-final-ingress-20261017/verify-source.py'
raw=bounded(P,7938,'8186f62cb05b263c051f1581db8fd2bacb6722506ef94588ace11749ef6e8635')
assert raw==bounded(D/'before/verify-source.py',7938,'8186f62cb05b263c051f1581db8fd2bacb6722506ef94588ace11749ef6e8635')
old="template=b.read(R/recipe['helper_template'],digest=recipe['helper_template_sha256']).decode();assert b.scrub(helper)==b.scrub(template)"
new="template=b.read(R/recipe['helper_template'],digest=recipe['helper_template_sha256']).decode();template=b.custody_source_template(template);assert b.scrub(helper)==b.scrub(template)"
assert raw.decode().count(old)==1
current=raw.decode().replace(old,new,1)
assert current.replace(new,old,1).encode()==raw
helper=R/'docs/evidence/next-big-batch-source-binding-20261017/guard_payload.py'
helper_raw=bounded(helper,18420,'1641b57088ed77b78631be8843c0f074ea09e04f591811b129ddea709bbc11f9')
ns={'__file__':str(helper),'_AUTHENTICATED_PROGRAM':helper_raw};exec(compile(helper_raw,str(helper),'exec'),ns)
old_guard='guard=runpy.run_path(str(guard_path))'
new_guard='guard=_accepted_guard_extensions(R)'
assert current.count(old_guard)==1
current=current.replace(old_guard,new_guard,1)
assert current.replace(new_guard,old_guard,1).replace(new,old,1).encode()==raw
exec(compile(current,str(P),'exec'),{'__file__':str(P),'__name__':'__main__','_accepted_guard_extensions':ns['accepted_guard_extensions']})
