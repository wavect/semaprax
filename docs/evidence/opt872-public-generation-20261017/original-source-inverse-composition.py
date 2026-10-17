"""Finite whole-source composition; never changes a legacy table or repository."""
from pathlib import PurePosixPath
import hashlib,json,subprocess

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def checked_path(path):
    p=PurePosixPath(path)
    if p.is_absolute() or '..' in p.parts or str(p)!=path:
        raise ValueError('noncanonical repository path')
    return path

def blob(repo,head,path):
    checked_path(path)
    entry=subprocess.check_output(['git','ls-tree',head,'--',path],cwd=repo,text=True).strip()
    if not entry or entry.split()[0] not in ('100644','100755'):
        raise ValueError('missing/nonregular immutable Git blob: '+path)
    return subprocess.check_output(['git','show',head+':'+path],cwd=repo)

def forward(original,row):
    text=original.decode('utf-8')
    if sha(original)!=row['sha256']:
        raise ValueError('original archive SHA mismatch')
    for edit in row['edits']:
        if text.count(edit['before'])!=1:
            raise ValueError('legacy forward multiplicity: '+row['path'])
        text=text.replace(edit['before'],edit['after'],1)
    return text.encode('utf-8')

def undo(checkpoint,row,original):
    text=checkpoint.decode('utf-8')
    for edit in reversed(row['edits']):
        if text.count(edit['after'])!=1:
            raise ValueError('legacy inverse multiplicity: '+row['path'])
        text=text.replace(edit['after'],edit['before'],1)
    if text.encode('utf-8')!=original or sha(original)!=row['sha256']:
        raise ValueError('complete original mismatch')
    return original

def project(current,row):
    """Authenticate complete input before returning a complete named checkpoint."""
    if current!=row['current'].encode('utf-8') or sha(current)!=row['current_sha256']:
        raise ValueError('complete current input mismatch: '+row['path'])
    checkpoint=row['checkpoint'].encode('utf-8')
    if sha(checkpoint)!=row['checkpoint_sha256']:
        raise ValueError('complete checkpoint SHA mismatch: '+row['path'])
    return checkpoint

def compose(repo,capture_head,checkpoint_head,table_path,candidate):
    """candidate contains only explicitly checked literal edits at capture_head.

    The caller retains actual build/producer/wire validation duties. This proves
    source history, not producer qualification, current HEAD, or pin permission.
    """
    if subprocess.run(['git','merge-base','--is-ancestor',checkpoint_head,capture_head],cwd=repo,capture_output=True).returncode:
        raise ValueError('checkpoint is not capture ancestry')
    table=blob(repo,capture_head,table_path)
    if table!=blob(repo,checkpoint_head,table_path):
        raise ValueError('legacy table changed between explicit domains')
    legacy=json.loads(table)
    if not isinstance(legacy,list) or not 0<len(legacy)<=64:
        raise ValueError('finite legacy source rows required')
    seen=set();rows=[];archives={}
    for old in legacy:
        path=checked_path(old['path'])
        if path in seen:raise ValueError('duplicate source path')
        seen.add(path)
        archive_path=str(PurePosixPath(table_path).parent/PurePosixPath(checked_path(old['archive'])))
        original=blob(repo,capture_head,archive_path)
        if original!=blob(repo,checkpoint_head,archive_path):
            raise ValueError('legacy archive changed')
        checkpoint=blob(repo,checkpoint_head,path)
        if forward(original,old)!=checkpoint:
            raise ValueError('checkpoint Git blob differs from complete forward replay: '+path)
        undo(checkpoint,old,original)
        raw=blob(repo,capture_head,path)
        current=candidate.get(path,raw)
        row=dict(path=path,current_sha256=sha(current),checkpoint_sha256=sha(checkpoint),current=current.decode('utf-8'),checkpoint=checkpoint.decode('utf-8'),capture_source_sha256=sha(raw),legacy_inverse_count=len(old['edits']))
        if undo(project(current,row),old,original)!=original:raise ValueError('whole chain failed')
        rows.append(row);archives[archive_path]=original
    return dict(schema='semaprax.complete-source-projection.v1',status='SOURCE_COMPOSITION_ONLY_NOT_PRODUCER_OR_CURRENT_QUALIFICATION',capture_head=capture_head,checkpoint_head=checkpoint_head,legacy_table_path=table_path,legacy_table_sha256=sha(table),rows=rows),table,archives
