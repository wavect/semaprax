# Project Manifest v5 and Useful Data Command v2

Audience: language users, tool authors, and compiler contributors.

Status: locally evidenced. Hosted promotion, safe Windows npm publication,
registry publication, and release promotion remain open, so the completion
claim remains Partial.

## Closed fixed-adapter profile

Project v5 is additive: v1-v4 canonical manifests, package bytes, carriers,
and behavior remain unchanged. Its canonical manifest has exactly eleven
assignments:

```toml
schema = "semaprax.project.v5"
name = "spxgrep"
version = "0.1.0"
profile = "useful-data-command.v2"
entry = "spxgrep.app"
sources = ["src/app.spx", "src/tests.spx"]
web_exports = ["spxgrep.contains"]
command = "spxgrep.contains"
input = "stdin-bytes+one-utf8-arg.v1"
capabilities = ["process.args.read", "process.stderr.write", "process.stdin.read", "process.stdout.write"]
tests = ["spxgrep.tests"]
```

`web_exports` contains exactly the command stable ID. The command has exact
signature `(borrow Slice<u8>, borrow Slice<u8>) -> bool`; its SEMAPRAX closure
still admits only `process.stdout.write`, and the test closure remains
effect-free. The four manifest capabilities are the complete authority of the
fixed process adapter: one argument read, stderr failure reporting, stdin read,
and the final stdout write. They do not add language-level argv, stdin, or
stderr operations and cannot be reordered, omitted, or widened.

The adapter accepts exactly one UTF-8 argument as the needle and arbitrary
binary stdin as the searched input. Their cumulative size is bounded to 65,536
bytes with checked accounting. The adapter invokes the function selected by
the authenticated command stable ID; native Project builds do not route this
profile through the legacy `main` entry point.

## Native and Wasm/Node behavior

The same target-neutral command admission authenticates the selected signature,
closure, stdout effect and permit, one-write path bound, and external-slice
provenance before either target is emitted. A match may seal one authenticated
external Slice parameter as the semantic transcript and exits 0 after a
successful physical flush; the `spxgrep` fixture selects the original stdin
slice. A non-match publishes no transcript and exits 1. Invalid adapter input,
semantic or invariant failure, or stdout write/flush failure exits 2 and cannot
be reported as semantic success.

The generated native process adapter is fixed rather than user-programmable.
On Windows it uses `wmain`, rejects invalid UTF-16 while converting the single
argument to UTF-8, and puts stdin and stdout in binary mode. On Unix it
validates the argument bytes as UTF-8 and treats `SIGPIPE`/broken stdout as an
adapter failure. Physical stdout is fallible and may have emitted a prefix
before a write or flush failure; success-only publication describes the sealed
semantic transcript, not atomic or durable operating-system output.

The existing Wasm/Node route retains its fixed guest transcript and performs
the same binary-stdin, cumulative-limit, result, transcript, and exit mapping
for admitted well-formed JavaScript scalar strings. Node exposes an
already-decoded argument string, so this route does not attest the original
Unix argv bytes; raw-byte UTF-8 validation belongs only to the native Unix
adapter. This is bounded native and Wasm/Node command parity over their stated
input boundaries, not general process I/O.

## Package evidence

The npm build uses the new independently replayed
`semaprax.project-npm-build.v4` carrier. Its seven-artifact inventory remains:

1. `app.wasm`
2. `semaprax.js`
3. `semaprax.bindings.js`
4. `semaprax.bindings.d.ts`
5. `semaprax.command.json`
6. `semaprax.command.js`
7. `package.json`

`semaprax.command.json` uses schema `semaprax.useful-data-command.v2` and binds
the exact input profile, four adapter capabilities, success-only transcript
policy, 65,536-byte bound, one-write-per-path maximum, bool result, exits
0/1/2, and Wasm digest. Independent replay rebuilds the semantic recipe,
selected command, Wasm bytes, metadata, artifact inventory, and v4 payload
digest. It grants no publication authority.

## Nonclaims

This profile does not add general or language-level stdin, argv, stderr, files,
directories, environment access, networking, child processes, callbacks,
WASI, streaming, multiple writes, dependencies, lockfiles, signing, or
provenance. It does not claim atomic physical stdout, executable-byte
determinism, Windows-safe npm publication, registry publication, hosted CI, or
full v0.2 completion. The generated native runner ABI is internal and is not a
stable public ABI.
