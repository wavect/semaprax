# Private startup configuration evaluation

`compiler_config_startup_qualification.py` supports the separate qualification
runner. It does not launch or initialize a server, widen the existing Session,
construct ExecutionAuthority, invoke a model, or grant permission to execute.
The retained ConfigReadParams and ConfigReadResponse schemas are copied whole
and checked against literal source hashes before validation. Runtime unknown
fields and the narrower finite configuration predicates both remain enforced.

The runner owns the actual private process, stdio descriptors, initialization,
request IDs and complete bounded raw JSONRPC request/result transcripts. It must
send `config/read` using `startup_request(plan)`: the exact workspace cwd and
`includeLayers: true`. Specifying cwd is necessary because the retained schema
says project layers are resolved for that directory. No thread/turn/model request
is part of this startup evaluator.

Before the actual request the runner calls `acquire_configuration(plan)`, which
reads the real named configuration through the original protected no-follow
reader and checks the complete inventory size and hash. It passes those bytes,
the exact sent params and the actual decoded result to `evaluate_startup` with
explicit model/effort. The evaluator checks both retained schemas and the full
existing single-layer resolver, returning canonical params/result hashes and
the resolved candidate. The runner retains original wire hashes separately;
canonical serialization is not the received wire representation.

After actual config/read the runner reacquires configuration and calls
`require_same_configuration(before, after)`. This compares full file identity,
metadata, complete bytes, inventory hash, path and contract. These are endpoint
observations, not proof that the file was protected between observations or
that any command came from the compiler. The snapshots hold no descriptors and
are not authority leases. Caller-supplied snapshots, response dictionaries,
connection names, nonces and matching hashes cannot mint authority.

An unexpected actual response (additional managed/system/project/default layer,
unknown configuration/default field, missing origins, changed source or runtime)
refuses. The accepted static schema does not prove that this installed runtime
will emit the finite supported shape. Actual startup/cache/auth/state behavior
and the observed layer set must be qualified by the parent runner; this module
does not guess precedence or discard unexpected default fields to force a pass.
Protected provisioning, actual runtime policy construction/inclusion, effective
environment and loader identity, socket/FD origin, no-descendant enforcement and
complete child settlement remain separate obligations. The existing admission
boundary is still closed.

Pure controls use synthetic result dictionaries and mock only the protected
configuration read when testing acquisition. They make no actual server, RPC,
sandbox, child, provider or model invocation and do not manufacture live grants.
