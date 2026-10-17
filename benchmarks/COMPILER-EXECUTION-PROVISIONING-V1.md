# Shared-runtime provisioning preflight v1

This opt-in support module does not change any campaign entry point or original
retained receipt v1. It does not grant execution authority or close #869/#606.
Both SEM and TS must use the same new matched configuration before a future
backend can claim qualified execution. No model, compiler or runtime is launched.

`compiler_execution_authority.preflight(contract_path, expected_sha256)` validates
a separately supplied literal contract digest, duplicate-free UTF-8 JSON, exact
versioned fields, declared artifact inventory and no-follow bounded descriptor
reads. `ProvisioningPlan` is immutable static data, not a capability. Its
`unresolved` obligations remain explicit. `admit` always refuses: no effective
policy/enforcement proof format is recognized in this version. `ExecutionAuthority`
cannot be constructed through its public constructor; its reserved methods
`check_current`, `check_private_path` and `authorize_cwd` refuse as well. A Python
object or an internally consistent manifest is never an OS isolation proof.

The closed contract has these fields:

- `schema`: `semaprax.compiler-execution-provisioning.v1`.
- `runtime_binary`, `compiler_binary`: absolute normalized inventory paths.
- `inventory`: unique `{path, sha256, size, role}` rows. Roles are exactly one
  runtime, exactly one compiler, and declared dependency/configuration/schema
  files, including at least one configuration and schema. Each file's basename
  is its lowercase SHA256; positive integer size is at most 256 MiB. The caller
  must inventory the complete actual load/configuration closure. This module
  cannot independently establish that declared closure's completeness.
- `workspace_root`, `server_cwd`: the same candidate directory; `evidence_root`
  lies outside every declared agent writable root. `agent_writable_roots` lists
  all declared roots, not merely the workspace. Future actual admission must
  prove there are no additional effective write/rename scopes.
- `sandbox_policy`: only installed `workspaceWrite` shape, explicit writableRoots
  exactly matching the declared inventory, networkAccess false, excludeSlashTmp
  true, excludeTmpdirEnvVar true. This is a declaration, not enforcement evidence.
- `server_env` and identical `command_env`: exactly PATH, HOME, TMPDIR, LANG,
  LC_ALL, TZ. Locale is C/C/UTC. PATH is one absolute protected directory; no
  inherited search path, shell expansion, dynamic-loader variable or caller
  override is accepted. Actual server-computed environment continuity remains
  unproved because command environment overrides merge with server state.
- `approval_policy`: never; `alternate_execution`: disabled;
  `enforcement_proof`: null. Any supplied proof/qualified flag refuses.

Server argv is fixed to the pinned runtime, `app-server --listen stdio://`.
No caller flags or configuration overrides are accepted. Future proof must
account for managed/user/config loading; these argv bytes do not suppress it.

The contract, artifacts, environment directories and evidence directory undergo
root-owned/non-group/world-writable checks on every no-follow ancestor. Candidate
roots are no-follow directories with protected parents, allowing writable contents
without granting candidate-root replacement. These are preliminary POSIX mode
checks only: ACLs, read-only mount identity, sandbox-excluded authority, loader
closure, socket restrictions, runtime policy and continuity still require an
actual recognized enforcement implementation. Preflight does not write/chmod/
mount/provision anything or require administrator privileges. An administrator
would separately provision a complete private content-addressed store; ordinary
same-UID chmod0555 files cannot qualify. No hashes or nonce alone prove origin.

Pure focused checks:

```
PYTHONDONTWRITEBYTECODE=1 TMPDIR=/Volumes/Semaprax/runtime-tmp python3 -m unittest discover -s benchmarks -p test_compiler_execution_authority.py -v
```

These exercise schema refusals and local descriptor acquisition. They never
manufacture an ExecutionAuthority, launch an app server, or claim live policy
qualification. Actual transport/broker integration and enforcement qualification
remain separate pending work.
