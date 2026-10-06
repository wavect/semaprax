# What Semaprax verifies and what it does not

After this page you can read the output of the registry, audit, release and
evidence commands without claiming more than they prove.

The commands themselves are in [Shipping](../projects/shipping.md). This page
states their trust limits in one place. The common rule: **evidence is data, not
permission.** A digest binds exact bytes. A passing check says those bytes match
what was checked. It never authorizes a write, a publication or a signature.

## Table of limits

| Surface | Checked | Not checked or not done |
| --- | --- | --- |
| `registry` commands | Digests bind subject bytes; names are not `std.*`; versions are immutable; duplicates refused (`SPX-PKR602` to `PKR605`). | No network. No default registry or search path. No signature check: an entry's `signature.identity` is an unverified claim, and a forged signature passes this layer. `publish` only prints what would result. |
| `fetch`, `resolve` | Each subject is replayed before it is cached; selection uses only the local cache. | Nothing downloads. A build does not yet link resolved external dependencies. |
| `audit verify` | Required object types per profile, associations, subject bindings, every object's digest, role and revocation policy. | With no `--trust-roster`, signatures are checked against policy only, and the output says so. Semaprax signs nothing and contacts no transparency log. |
| `release verify` | Manifest digest, provenance binding, every named archive re-hashed. With signed material, the Sigstore check offline against the trusted-root bytes you supplied. | A directory with no signed material prints `VERIFIED UNSIGNED RELEASE`. A pass does not say the roots are current or that you downloaded from an official place. |
| `doctor verify-release` | The same, but it requires signed material and your `--trusted-root-sha256`. | The digest must come from a channel you trust. One copied from the release directory proves nothing (`SPX-Z707` on a wrong one). |
| `verify`, `*-evidence` | Evidence is replayed against exact source and patch bytes. | The result is proof data. Applying still needs the `apply` command, and a stale source (`SPX-G409`, `SPX-G530`) is refused. |
| `workflow dispatch` | Whether a declared request is inside a declared allow-list. | It performs nothing. It records a decision. |
| Hot-reload plans | A candidate passes the full project check before it can swap in. | A plan has `"authority": "none"`; only `activate` swaps, between invocations. |
| Harness providers | Permission requests in a descriptor are checked against grants in your harness home, bound to the descriptor, adapter and upstream digests. | A changed or widened adapter loses its grant. Adapters do not get ambient `PATH` or `$HOME` access. |

## Read these four words

- **`authority`: `false` or `none`.** The document is information. Having it
  changes nothing you may do.
- **`nonclaims`.** A list of things the producer states it does not establish.
  Read it before you quote the result.
- **Stale.** The source or revision moved since the evidence was made. Re-run the
  producing command. Do not edit the evidence.
- **Fail closed.** The command stops with a code and writes nothing rather than
  guess.

## What to say in a review

Say what the command proved, with its scope: "`release verify` re-hashed all five
archives against `release-manifest.json`", not "the release is signed". When a
capsule or release is unsigned, say unsigned. Local test evidence is local; hosted
or physical-device evidence needs its own record. The same honesty applies to
this handbook: where a feature is private, experimental or main-only, the page
says so.

Specs: [Audit Capsule v1](https://github.com/wavect/semaprax/blob/main/docs/AUDIT-CAPSULE-V1.md),
[Release signing policy](https://github.com/wavect/semaprax/blob/main/docs/RELEASE-SIGNING-POLICY-V1.md),
[Registry trust](https://github.com/wavect/semaprax/blob/main/docs/PACKAGE-REGISTRY-TRUST-V2.md).
