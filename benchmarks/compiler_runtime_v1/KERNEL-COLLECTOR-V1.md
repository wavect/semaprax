# Darwin kernel collector candidate v1

This source implements an explicit future `run_qualification` action. Importing
it starts no process. Neither the campaign, old Session, nor `admit` calls it.
No ExecutionAuthority can be constructed; collected JSON never grants one.

The action accepts an exact v2 provisioning contract, explicit protected TCB
roots, declared sandbox launcher and build compiler, model and effort, and a
fresh evidence directory. It builds the fixed native probe using an owned
bounded producer and records actual command, environment, status and image hash.
The native source is literal-hash bound. Fixed harness work is separate from
model tokens. No native compile, app-server startup, RPC or kernel probe has
been run for this source checkpoint.

Two collection modes are distinct. `root-dac-empty-acl-v1` retains the original
strict root-protected preflight. Every ancestor must be root-owned; a root-owned
leaf under the current UID501-owned `/Volumes/Semaprax` fails. This candidate
changes no ownership and performs no sudo action. The separate
`ssd-kernel-probe-candidate-v1` mode reads exact bounded no-follow inventory and
holds ancestor/leaf handles with UID/mode/empty-ACL and rebinding checks. These
are drift and surface checks only. Same-UID ownership or chmod cannot protect
the store from the agent and cannot grant authority.

A candidate outer Seatbelt envelope wraps the actual app-server argv. It denies
file writes under explicit disjoint TCB roots and to their ancestor entries and
the named configuration path. Ordinary workspace paths are excluded from these
barriers. Whether the kernel actually denies write, rename and replacement
through every inherited agent-tool path must be measured; profile text alone
does not prove inheritance, intersection or absence of privileged deputies.
The app-server still receives the exact declared workspace sandbox policy.
There are no thread/turn/model operations. The private transport admits only
initialization, exact config/read and predeclared probe commands, with complete
bounded raw wire and stderr, strict IDs, stream truncation refusals and explicit
server process-group settlement.

Startup evaluation reuses the pinned installed ConfigReadParams/Response
schemas and complete single-user-layer resolver. In SSD mode a distinct local
candidate snapshot surrounds the actual RPC; endpoint equality is not an
in-between protection lease or origin proof. The original strict snapshot
helpers remain unchanged. Parameters and canonical result hashes remain
separate from the actual wire transcript.

The instrumented probe profile runs a fixed probe, not the compiler. Its
explicit descriptor-observation reads and native exec target are recorded as
such. Cases cover ordinary read/write, private evidence denial, environment,
FDs, fork, foreign exec, network, Unix sockets, signal and timeout. Permission
denials require EPERM/EACCES; missing paths or resource failure do not count.
Native diagnostic/status/error streams remain whole. Direct PID disappearance
is explicitly narrower than whole descendant settlement.

Before any grant can be implemented, actual controls still need to establish:

- paired direct app-server write refusal and ordinary workspace success;
- actual TCB file/ancestor replacement refusal through all agent-tool routes;
- complete loader/interpreter/config closure and protected runner lifetime;
- actual exact compiler invocation, no-fork enforcement, timeout/client-loss
  attribution and descendant settlement, including escaped process groups;
- a live enforcement lease, receipt binding to that lease, and common matched
  effective environment/config/loader/policy fingerprints across both arms.

Private directory creation uses no-follow parent handles and fresh mode0700
leaves. Later path use is not yet a same-UID lifetime lease; qualification must
close runner-evidence/config rebinding and tool escape paths before admission.
Disposable qualification workspace markers remain isolated from authored files.

Darwin ACL constants/signatures follow the retained local SDK `sys/acl.h`:
ACL_TYPE_EXTENDED=0x100, ACL_FIRST_ENTRY=0, acl_get_fd_np/acl_get_entry/acl_free.
Unknown, invalid or nonempty ACLs refuse. The empty-list return convention is
from Apple's acl_get_entry manual (success0; no entry/error-1 and errnoEINVAL):
https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/acl_get_entry.3.html
Actual kernel/ACL behavior remains untested by the pure source controls.

Lifecycle hardening: group ownership is observed while the direct Popen child
is unreaped. After wait/poll has reaped that leader, settlement performs only
bounded absence observations; it never signals a possibly reused numeric group.
Residual/unknown groups refuse rather than receive guessed cleanup authority.
Producer setup failures still settle the owned group and close pipes, and
partial startup cleanup forces nonblocking drains. Cleanup refusal gets its own
terminal evidence, including after otherwise successful probes; an evidence-write
failure cannot replace an earlier selected failure. These are pure-tested source
controls, not actual process-group or kernel qualification.
