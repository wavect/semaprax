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
current_binding="helper_raw=bounded(helper,22191,'1e71fcda59e1f6e72b443d2ab21088c1d7dd0af4d6cdcf3e74b7e893756c1ac4')"
assert program.count(old_binding)==1
program=program.replace(old_binding,current_binding,1)
old_ingress_sha='799eee227e9d57fb000d75cbf018e0faf3b9bc03cd592f0405d3050479a12871'
current_ingress_sha='f8e49cc014614d85907f81d504914cabc478e278a8217f3a0956339a2c107b9a'
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
