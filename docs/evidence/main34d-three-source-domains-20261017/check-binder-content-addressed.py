"""Exercise the actual binder archive admission; no endpoint or Rust writes."""
import hashlib, importlib.util, json, os, tempfile
from pathlib import Path

assert os.environ['TMPDIR'] == '/Volumes/Semaprax/runtime-tmp'
here = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('actual_main34d_binder', here/'bind-inputs.py')
module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
with tempfile.TemporaryDirectory(prefix='main34d-content-addressed-', dir=os.environ['TMPDIR']) as temporary:
    root = Path(temporary); files = {}; before = b'complete prior\n'; current = b'complete current\n'
    first = module.queue_current(files, root, before)
    assert first.name == 'meta-' + hashlib.sha256(before).hexdigest() + '.txt'
    second = module.queue_current(files, root, current)
    assert first != second and files[first] == before and files[second] == current
    path = root/second; path.parent.mkdir(parents=True); path.write_bytes(current)
    assert module.queue_current({}, root, current) == second
    path.write_bytes(current + b'\n')
    try: module.queue_current({}, root, current)
    except AssertionError: pass
    else: raise AssertionError('reused archive drift was admitted')
    path.write_bytes(current)
    try: module.queue_current({second:b'forged'}, root, current)
    except AssertionError: pass
    else: raise AssertionError('conflicting pending archive was admitted')
    path.write_bytes(current)
    before_stat=path.stat()
    module.write_current(root,second,current)
    assert path.stat().st_mtime_ns==before_stat.st_mtime_ns and path.stat().st_ino==before_stat.st_ino
    path.unlink()
    outside=root/'outside'
    for target in [outside, root/'missing-target']:
        if target==outside:outside.write_bytes(current)
        path.symlink_to(target)
        for operation in [lambda:module.queue_current({},root,current), lambda:module.write_current(root,second,current)]:
            try:operation()
            except AssertionError:pass
            else:raise AssertionError('symlink archive admitted')
        path.unlink()
    module.write_current(root,second,current)
    assert path.read_bytes()==current
inverse = json.loads((here/'binder-content-addressed-inverse.json').read_bytes())
raw = (here/'bind-inputs.py').read_bytes()
old = (here.parents[2]/inverse['original_archive']).read_bytes()
assert len(raw) == inverse['current_len'] and hashlib.sha256(raw).hexdigest() == inverse['current_sha256']
assert len(old) == inverse['original_len'] and hashlib.sha256(old).hexdigest() == inverse['original_sha256']
assert inverse['inverse'] == [{'at':0,'remove_sha256':inverse['current_sha256'],'remove_len':len(raw),'insert_archive':inverse['original_archive']}]
print(json.dumps({'status':'PASS_PYTHON_ARCHIVE_ADMISSION_ONLY','positive':5,'negative':6,'complete_old_binder_inverse':True,'Rust_or_endpoint_or_common_execution':False}))
