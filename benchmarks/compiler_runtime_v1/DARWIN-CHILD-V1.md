# Darwin compiler child v1 — private implementation, unqualified

`compiler_darwin_child.py` is a concrete protected-child entry intended below the
same private runtime's `command/exec`. It is not a runner-local compiler replay
and mints no authority. The existing Session still rejects wrapper executables;
ExecutionAuthority/admit remain unchanged and refused. No OS probe was run.

The closed child CLI accepts exactly `--contract PATH --contract-sha256 SHA
--request-json JSON`. Request fields are argv, input_files,
publication_directory and sandbox_launcher. These are requests, not grants. A
future qualified runtime caller must bind the exact command, interpreter/module
and contract bytes before admitting this wrapper. The interpreter and its entire
startup/import/loader environment must already be protected; resetting the final
compiler environment cannot cure an injected launcher interpreter.

Preparation validates the exact retained codec argv, fixed project `.`, terminal
runner seal nonce, captured manifest/schema/input membership and a dedicated
one-component publication directory under the trial root. It preserves the
original argv. The sandbox launcher must be a content-addressed protected
provisioning dependency, not an ambient PATH search or arbitrary host executable.
A provisioned copy of sandbox-exec and its signature/platform behavior require
qualification; the static dependency role alone cannot establish its semantics.

`profile_source` constructs versioned deny-default Seatbelt source with exact
read files, exact ancestor metadata reads, one compiler exec target and the new
publication subtree (including compiler atomic temporary publication files). It
imports no profile and grants no process-fork/network/socket/mach-lookup rule.
Literal path quoting is deliberately restricted. This is candidate source, not a
kernel-validated profile or proof that every omitted operation is denied on the
actual OS. Dependency/startup requirements may cause refusal and may not be
silently widened. The resolver's declared-path algebra is separate from actual
kernel policy semantics.

The irreversible entry preflights the protected contract and rechecks every
inventory file's bytes/mode, acquires the cwd by no-follow descriptors, creates
an exclusive publication directory, selects that held cwd, binds stdin to the
OS null device, closes extra descriptors and requires pipe stdout/stderr.
It uses `execve` with a new six-key environment and inline `sandbox-exec -p`
source, then the exact compiler argv. Thus it introduces no additional launcher
child PID or shell. Inline source avoids a mutable per-request profile file.
It cannot establish ACL/mount/ancestor continuity, protection of the interpreter,
absence of native threads or the true provenance of pipe endpoints by itself.
Qualified /dev/fd namespace and OS null-device semantics are required. Failed
exclusive directory acquisition does not adopt or erase existing contents.

`observe_direct_exit` retains the actual bounded connection-owned exit and
streams; its whole-child label is fixed UNQUALIFIED_NO_FORK_ENFORCEMENT and cannot
be set by a constructor argument. A direct exit is not descendant settlement.
The future live qualifier must enforce no compiler descendants and verify the
runtime direct-child timeout/signal/client-loss lifecycle, or provide genuine
complete supervision. Runtime PG scans and final stream EOF cannot replace that
proof. No static record/boolean can promote this observation to execution grant.

Remaining integration: prefix lane owns finite config/environment/policy
resolution and profile-description validation. Its candidate must bind this
exact source and actual profile to a supported effective agent scope. Only after
protected provisioning, actual paired policy/env/loader/no-fork/settlement
controls pass may a real qualifier construct a live grant and Session admit the
wrapper route. Current campaign CLI, labels and old receipt algorithms are
unchanged. Offline controls execute no child, sandbox, runtime, RPC or provider.
