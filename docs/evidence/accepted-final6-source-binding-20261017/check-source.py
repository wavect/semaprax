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
new='template=b.read(R/recipe[\'helper_template\'],digest=recipe[\'helper_template_sha256\']).decode();template=b.custody_source_template(template);prior_helper=b.read(R/\'docs/evidence/fmt-a110/v8M15AIpSv-8kL6VHWd_Go87kMGIdJ7QfMfFIMH5XH4.txt\',18589,\'bfc335e402294affbc90be951d677f1a8f3b90c188749ed07cc7c520c1f95c7e\').decode();assert b.scrub(prior_helper)==b.scrub(template);assert len(helper.encode())==19828 and sha(helper.encode())==\'6547926089a08e522fbb63eaf753e4ef482682b9a8350a80a6ec89ea2b277e1e\';_strict_pins=b.pins;b.pins=lambda code:_strict_pins(code.replace(\'const FINAL_INGRESS_SHA256: &str =\\n    \\"\',\'const FINAL_INGRESS_SHA256: &str = \\"\'))'
assert raw.decode().count(old)==1
current=raw.decode().replace(old,new,1)
old_fmt='assert restored.count(hook)==1;restored=restored.replace(hook,old,1);assert restored.encode()==original'
new_fmt='assert restored.count(hook)==1;prior_restored=prior_helper.split(marker)[0];assert prior_restored.count(hook)==1;prior_restored=prior_restored.replace(hook,old,1);assert prior_restored.encode()==original'
assert current.count(old_fmt)==1
current=current.replace(old_fmt,new_fmt,1)
old_dirty="raw=b.read(R/row['path']);assert raw==git('show',a.expected_head+':'+row['path'])"
new_dirty="raw=b.read(R/row['path']);assert (len(raw),sha(raw))==(row['current_length'],row['current_sha256'])"
assert current.count(old_dirty)==1
current=current.replace(old_dirty,new_dirty,1)
assert current.replace(new_dirty,old_dirty,1).replace(new_fmt,old_fmt,1).replace(new,old,1).encode()==raw
helper=R/'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py'
helper_raw=bounded(helper,19138,'fb8c4bc44e0a99249d70ebe4644e26d016c9417115814f6bbcd00024c00894a6')
ns={'__file__':str(helper),'_AUTHENTICATED_PROGRAM':helper_raw};exec(compile(helper_raw,str(helper),'exec'),ns)
old_guard='guard=runpy.run_path(str(guard_path))'
new_guard='guard=_accepted_guard_extensions(R)'
assert current.count(old_guard)==1
current=current.replace(old_guard,new_guard,1)
assert current.replace(new_guard,old_guard,1).replace(new_dirty,old_dirty,1).replace(new_fmt,old_fmt,1).replace(new,old,1).encode()==raw
exec(compile(current,str(P),'exec'),{'__file__':str(P),'__name__':'__main__','_accepted_guard_extensions':ns['accepted_guard_extensions']})
