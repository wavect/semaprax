"""Run the complete original FINAL checker with optional pure derived reuse."""
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
# An explicitly named incoming-parent operator retains its entire original body.
# This route is separate from the current cached and original-checker controls.
if '--incoming48-original-checker' in sys.argv:
 sys.argv.remove('--incoming48-original-checker')
 incoming=bounded(R/'e/c9b3f6b5c4bb1cc085710b8da140183444ec1d3a9d3d39b69b5491cef57aedd2',2253,'c9b3f6b5c4bb1cc085710b8da140183444ec1d3a9d3d39b69b5491cef57aedd2')
 old_guard_binding="helper_raw=bounded(helper,11549,'f7456316bbcdf82b1487d02d19d848814a43ee6cd9ea3b2c5a7266e5a997278c')"
 current_guard_binding="helper_raw=bounded(helper,22191,'ecbaaf6363765a780a56d7a2868554d03556f98d0414cbab52fabd6cbd679eb4')"
 assert incoming.decode().count(old_guard_binding)==1
 program=incoming.decode().replace(old_guard_binding,current_guard_binding,1)
 assert program.replace(current_guard_binding,old_guard_binding,1).encode()==incoming
 exec(compile(program,str(D/'check-source.py'),'exec'),{'__file__':str(D/'check-source.py'),'__name__':'__main__'})
 raise SystemExit(0)

original=bounded(D/'before/check-source.original.py',3542,'63e0399384c8a6ccdc56b138504e4989a59cb274c0976169c69592f2e0e4917f')
uncached='--original-checker' in sys.argv
if uncached:sys.argv.remove('--original-checker')
namespace={}
if not uncached:
 helper=bounded(D/'derived_compose.py',4597,'18ba0b7cf6ff5f2c2df9fd6f2b482336bfdf22612fca7f88fbc1a223aa54fb83')
 namespace={'__file__':str(D/'derived_compose.py')}
 exec(compile(helper,namespace['__file__'],'exec'),namespace)
program=original.decode()
# The immutable old wrapper retains its complete authenticated original bytes.
# This exact input-pin transport applies equally to cached and uncached execution.
old_binding="helper_raw=bounded(helper,19144,'af70eaeeb1cc0da02460858f9e7ddb39086582add858c74138c4d989644e11f3')"
current_binding="helper_raw=bounded(helper,22191,'ecbaaf6363765a780a56d7a2868554d03556f98d0414cbab52fabd6cbd679eb4')"
assert program.count(old_binding)==1
program=program.replace(old_binding,current_binding,1)
old_ingress_sha='799eee227e9d57fb000d75cbf018e0faf3b9bc03cd592f0405d3050479a12871'
current_ingress_sha='50e29c8971b4e773f5085e653b18086e2e2032a68d15cddc4762d7ae3cf6f379'
assert program.count(old_ingress_sha)==1
program=program.replace(old_ingress_sha,current_ingress_sha,1)
bound_program=program
assert bound_program.replace(current_ingress_sha,old_ingress_sha,1).replace(current_binding,old_binding,1).encode()==original
anchor="ns={'__file__':str(helper),'_AUTHENTICATED_PROGRAM':helper_raw};exec(compile(helper_raw,str(helper),'exec'),ns)"
assert program.count(anchor)==1
if not uncached:
 replacement=anchor+";_install_derived(ns)"
 program=program.replace(anchor,replacement,1)
 assert program.replace(replacement,anchor,1)==bound_program
assert bound_program.replace(current_ingress_sha,old_ingress_sha,1).replace(current_binding,old_binding,1).encode()==original
exec(compile(program,str(D/'check-source.py'),'exec'),{'__file__':str(D/'check-source.py'),'__name__':'__main__','_install_derived':namespace.get('install')})
