# Held-byte definition helper followup

Source-only followup to7b7b380a319ef9847a159c51190968e99ff8fff7.
The wrapper now creates a ModuleType with an explicit __file__ and executes
compile(raw, helperpath, 'exec') in its namespace, using ONLY the already
length/SHA-authenticated held helper bytes. No loader reopens the pathname.
Both script held() and definition bounded() compare opened-FD identity before/
after reading AND a post-read lstat of the same physical path, require a regular
non-symlink file, and match device/inode/size/mtime/ctime. This is snapshot
continuity, not a filesystem lease or broader authority claim.

Four complete7b preimages and exact finite byte inverses are retained. The
recipe preserves its complete7b prefix and adds one exact whole-wrapper edge.
The NEWMAIN binder's accepted wrapper and recipe SHAs are replaced with these
exact successor bytes; no mutable mask or wildcard hash was introduced. The
complete original verifier snapshot and definition-bindings.json remain byte-
exact, including the fixed6160-byte historical row.

Minimal pure checks restore all four exact7b Git inputs, reconstruct the whole
public wrapper from every original recipe edge, check helper/binder pins,
confirm held-byte execution and FD/post-path identity code, and parse Python
ASTs. No full verifier/binder main, common refresh, Rust test or Cargo ran.
Actual parent common refresh remains the next coordinated action.
