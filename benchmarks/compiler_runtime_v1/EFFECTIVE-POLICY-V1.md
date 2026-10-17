# Private finite configuration and policy checks

`compiler_effective_policy.py` is static validation for a future qualifier. It
constructs no `ExecutionAuthority`, changes no admission path, starts no process,
and changes no existing campaign. Its objects, caller dictionaries and hashes
carry no execution authority. This is not #869 completion or an effective runtime
policy measurement.

The supported configuration has exactly one enabled, unprofiled local `user`
layer. Its complete parsed TOML bytes must match both the complete root config
and layer config; every root key must have that exact layer name/version as its
origin. Unknown layers, config keys, profiles, disabled layers, missing origins,
non-boolean policy flags and environmental config guesses refuse. Model and
reasoning effort are explicit matched inputs. The finite profile admits only
approval `never`, one workspace-write root, excluded implicit temporary roots,
no network, disabled web search and disabled default apps. There is no generic
precedence implementation or remote/managed policy translation.

The held configuration bytes are length/SHA-bound to a configuration entry in a
`ProvisioningPlan`. The caller must eventually acquire them using protected
no-follow reads. A constructed plan or a matching SHA is not protection. The original v1 provisioning schema and all its code remain unchanged.
The separate `compiler_configuration_provisioning.py` adds the explicit opt-in
`semaprax.compiler-execution-provisioning.user-home.v2` route: exactly one named
configuration at `$CODEX_HOME/config.toml`, with `CODEX_HOME` explicitly equal to
the protected `HOME/.codex` directory in both complete environment maps. The
pinned installed runtime's retained `ConfigReadResponse` schema explicitly names
that loader path; app-server help names the default `~/.codex/config.toml` route.
The extension pins the exact runtime, schema and retained help digests, requires
the schema in the declared inventory, and admits no profiles or config flags.
It validates through the complete original v1 parser using a finite temporary
schema/env/config-path normalization, then restores the actual named row and
original contract SHA. No physical read uses the temporary synthetic sibling.
Actual preflight uses the original protected no-follow reader on the real named
file. All other artifact names remain content-addressed. This fixes the static
loader-path mismatch; actual runtime cache/auth/state write requirements and
managed/system layer absence still require qualification.

The path algebra models normalized absolute literal names only. Read/write roots
include their subtrees; file permissions and metadata-only permissions are exact
names; explicit denied subtrees override every grant. Network, Unix socket and
spawn permissions intersect by Boolean AND. A child's subtree grant must retain
every parent hole that intersects it. A parent file grant cannot cover an entire
child subtree. No lexical path comparison proves physical identity, aliases,
symlink/hardlink behavior, ACL/mount authority, a loader closure, descriptor
origin or an actual sandbox permission relation. Bounds are fail-closed. The current generated profile has no deny-hole rules,
so its executor description must have an empty deny list; otherwise a narrower
description could misrepresent the emitted publication subtree grant.

`validate_profile_description(plan, resolved, description, *, profile_bytes)`
accepts only the closed fields in the module. `argv` excludes the compiler and
includes the terminal stdout-seal nonce; the separate compiler field must match
the plan. Project is `.`, source and manifest are captured, and output is inside
one dedicated publication child of the workspace. The complete read inventory
is the union of protected inventory and captured inputs. Metadata-only names
are every non-root ancestor of those files and the publication directory.
Publication has read/write subtree grants; the compiler has the sole exec grant.
No network/socket/spawn grants are accepted. The environment equals the closed
plan command environment. A declared agent policy must include every resulting
executor permission in the finite model.

The complete supplied profile bytes (at most 262144 bytes) must match the exact
independently reproduced candidate grammar, not merely a caller's hash. This
checks the description against the launcher's proposed deny-default source;
it does not qualify Darwin's deprecated sandbox interface or its operations.
The profile grants no fork rule, but actual fork/vfork/spawn denial remains a
kernel qualification obligation. The sandbox launcher is an exact protected
inventory dependency; its copied signature/loader behavior is also unqualified.

`ResolvedConfig.normalized_config_sha256` replaces only the supported workspace
root slot with a role marker. `ProfileDescription.common_fingerprint_sha256` is
a conservative candidate digest, not a measured effective fingerprint: it keeps
raw profile SHA and invocation nonce, so differing dynamic profiles/nonces still
produce different hashes. Per-trial raw configuration and contract hashes remain
separate bindings. The effective environment/config/loader/authority fingerprint,
actual agent policy construction, protected TCB, actual pipe/FD origin, complete
child settlement and a live non-forgeable grant all remain required. No automatic
profile equivalence or policy inclusion against an unknown runtime is asserted.

Retained installed source evidence (no new runtime invocation):

- `ConfigReadResponse.json` SHA256 `4ec77e1a3eed746037149799c3b6bb8fe7b4b593e5930fc9f410e6282de9df50`, user layer description at lines 846–854.
- App-server help SHA256 `4226dbf080b9d3a620385fba067bd3b8ac3c76654f28c3bd4d92ae20df2b03d2`, config path lines 13–16.
- Installed runtime `codex-cli 0.163.0-alpha.10.2`, bytes SHA256 `1afb28c01d7d49e892bd75f11076b10f1451e34e0ee308a95e5ab7f69f9c84fe`, from the retained read-only receipt. Ownership/protection of that installed copy was not qualified.

Pure controls run with `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s
benchmarks -p test_compiler_effective_policy.py`. They use synthetic plans and
bytes, not mocked live grants, runtime calls or OS sandbox probes. Actual Rust,
build, provider, model and campaign execution remain outside this tranche.
