# Concrete minimum before ExecutionAuthority can be minted

Current `admit` always refuses. This document is an implementation/qualification
plan, not a recognized proof format, completed profile or authorization to probe.
No live probe was run. A signed/static JSON receipt, `qualified:true`, root-owned
metadata, a nonce, profile name or before/after hash cannot satisfy this boundary.

## Supported profile must be a finite implemented capability

Implement one recognized local profile, bound to the exact immutable runtime
binary, schema bytes and audited platform sandbox implementation. Its trusted
runner qualification function must establish the items below and mint a private
live grant itself. No deserializer or caller-supplied file can construct a grant.
`check_current` must revalidate the held profile/artifact/authority context at
launch and every operation; drift poisons that context. Before enabling admission,
execution receipts must also retain/bind the actual recognized qualification
proof and live supervisor/grant identity and lifetime, separately from the static
contract SHA. A proof digest is a reference, never authority itself. That live
proof/lease API and receipt binding are not implemented in the current refused
version. A saved receipt is useful review evidence but must not resurrect a live grant by itself.

The current Darwin host has `/usr/bin/sandbox-exec`, and the installed pinned
runtime has sandboxed `command/exec`. A concrete future path without an app-server
attestation field is a versioned, runner-owned, fully resolved local agent/executor
configuration, paired with an enforced narrower child profile. The runtime is
part of the trusted computing base: its supported policy factory and config
resolution must be reviewed and pinned, not inferred from a profile label or
`ThreadStartResponse.sandbox` alone. Managed/custom/remote policy that cannot be
fully accounted for must refuse this profile.

A runner-owned deny-by-default macOS Seatbelt child profile could additionally
enforce explicit read-only input/dependency paths, exactly one exclusive output,
closed process execution, and no network or Unix sockets. A sealed per-request
profile and `/usr/bin/sandbox-exec` would be protected runner artifacts; it would
run beneath the existing runtime sandbox, via trusted `command/exec`, not as an
outside-sandbox replay. This requires a future reviewed wrapper/profile extension
to the current exact-compiler argv scope; it is NOT currently implemented. Literal
policy path quoting, kernel semantics and startup/dependency reads require real
qualification. This profile enforces child limits without an attestation field;
it does NOT alone prove that an unknown agent policy is broader. Every granted
child access must also be justified against the complete owned agent policy.
No generic policy translator or ambient host `process/spawn` is needed or allowed.

## Required trusted qualification, before a paid campaign

1. Provision compiler, runtime, configuration, protocol, the actual complete
   dynamic loader/dependency closure, and any profile/probe wrapper in a protected
   store. Root-owned non-writable ancestors are preliminary checks only. Establish
   actual ACL/mount/link/rename/chmod authority and protection from all agent tool
   paths, not merely the shell. A separately provisioned read-only image or
   genuinely protected root-owned store is suitable only after these checks.
   The current user-owned application binary is not such provisioning.
2. Bind complete effective config/managed layers and local environment selection,
   not only requested flags. Enforce identical or executor-narrower filesystem
   roots/exclusions, cwd, network, Unix sockets, approval-never and compiler access.
   Account for apps/plugins/MCP/alternate tools/shell hooks and escalation paths;
   current app-server and codex-exec have different config-loading surfaces.
   Reject unknown layers or authority. The two declared trial-root contracts must
   share the qualified profile and runtime/compiler identities, with role-equivalent
   scope. Bind a common normalized effective authority fingerprint covering server/
   command environment, HOME/config/managed layers, dependencies and loader closure.
   The current two-contract slots and fixed_policy helper do not perform that
   comparison and are not a matched effective-policy proof. Do not silently replace the existing CLI benchmark configuration.
3. Establish the actual child environment and loader resolution. Command env is
   an override merged with server-computed values, not an empty environment.
   Prove that no hidden PATH/current-directory/loader injection or inherited socket,
   descriptor, credential or mutable library grants additional authority. Handle
   only the closed declared environment; unknown additions require refusal or a
   newly reviewed narrower rule. Static dependency lists are not loader-closure
   proof. Runtime/probe output is trustworthy only on the private connection from
   already protected executable code; agent-written assertions are not evidence.
4. Execute a fixed root-provisioned probe corpus through the actual supported
   sandbox policy factory and command lifecycle. Prove allowed source/compiler/
   loader/output operations and denial of protected-store/evidence/config writes,
   metadata changes, ancestor rename, symlink/hardlink attacks, excluded files,
   foreign cwd/root, network and Unix sockets, loader injection, foreign executable
   and inherited capability use. Cover both the agent child policy and executor
   child policy; do not use unsandboxed `thread/shellCommand`/`process/spawn` as
   surrogate proof. If the exact agent child path cannot be exercised or audited
   without unsupported machinery, that is a blocker, not a successful probe.
5. Qualify real failure, timeout, signal and whole-child settlement, including
   detached/new process groups and late stdout. A final response's direct exit
   status plus this client's own server-PG check is not proof of complete descendant
   cleanup. The recognized profile must enforce a lifecycle it can actually settle
   or refuse. Preserve the observed status, unknown values and paid attempt cost;
   never retry into a successful label or infer a missing status.
6. Qualify one actual protected compiler invocation, exact argv/cwd/policy/env and
   private runtime status/stream ordering, then no-follow retained bytes/seal.
   Exercise publication conflict, swapped/truncated/duplicate output, wrong nonce,
   stdout failure after publication and all old success/refusal baselines. Retain
   the complete original/resolved/mixed inventories independently. Only then may
   the implemented qualifier mint a live ExecutionAuthority for this exact scope.

This bounded local path is implementable in principle with owned platform
profiles and a trusted runtime. Effective policy, protected provisioning, loader
closure and full child lifecycle are concrete missing implementations/actual
qualifications today. Until they exist, unsupported admission must remain refused.
Afterward, parent must explicitly admit fresh matched configuration/calibration
and provider spend; neither these pure controls nor a local origin proof closes
#606 or establishes independent repeat, billed cost or token/ratio advantage.
