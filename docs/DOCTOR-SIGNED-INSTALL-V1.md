# Doctor signed generation install v1

Status: private Unix implementation contract; not ordinary CLI activation or
production support.

## Boundary

This contract turns one independently authenticated, unpacked Linux doctor
release into one immutable local generation. The caller supplies the release
version, commit, target triple, architecture, target, selector and Ed25519
public key independently. Distribution metadata never supplies its own trust.

The store is an existing absolute normalized current-euid-owned mode-0700
directory on a trusted local filesystem. Its complete ancestor chain and the
source directory must be quiescent against same-principal mutation for an
invocation. The implementation holds and rechecks every directory component,
uses no-follow handle-relative operations, and takes a nonblocking exclusive
lock on the held store directory. Missing identity, durability or locking
facilities reject; there is no pathname-only fallback.

## Layout and publication

Completed entries are `generation-<64 lowercase SHA-256 hex>`. The digest uses
domain `semaprax.doctor-installed-generation.v1\0` and binds, in canonical
inventory order, every name, executable bit, exact length and byte of all nine
release members. Installation verifies the source before effects, reads every
single-link regular member through a held descriptor, rechecks source identity
and bytes, creates one exclusive `.stage-<digest>` directory, writes exclusive
0600/0700 members and settles every file and directory. It independently
replays the staged signed release before one no-replace rename. Existing
generation names reject; they are never adopted or overwritten.

`ACTIVE` is exactly one lowercase generation digest plus newline in a
single-link mode-0600 file. Activation and rollback hold the cooperative store
lock, require an exact expected-current value, independently replay the selected generation,
settle `.ACTIVE.stage`, recheck the expected current value and target, then
rename and settle the store. This is an expected-current transition among
cooperating writers under the documented quiescence precondition, not a kernel
compare-and-swap against an uncooperative same-principal writer. Failure before the pivot preserves the old active
generation. Failure after a rename is explicit uncertainty and must be resolved
with `inspect_active`, never blind retry.

Recovery removes only a complete stage whose name digest, exact inventory and
signed release all independently replay under caller-supplied expectations. It
holds every member and revalidates the complete name-to-held-identity inventory
in one pre-effect pass. Unix unlink has no inode-CAS; the trusted/quiescent
same-principal precondition remains necessary after that final check.
Partial, surplus, substituted, ambiguous or ACTIVE stages are preserved and
fail closed. Recovery never deletes an unauthenticated entry.

## Bounds

- store paths: 4,096 bytes and 64 normalized components;
- generations: caller-selected 1 through 32;
- at most one inert generation stage and one ACTIVE stage;
- release inventory: the existing exact nine files;
- each member: at most 512 MiB, with the existing narrower manifest/capsule
  limits retained by release replay.

## Nonclaims and promotion stop

The API does not unpack an archive, discover a store, execute an image, grant
cgroup or namespace authority, activate `semaprax doctor`, authenticate the
kernel or build host, support network filesystems, defend against root/admin,
or implement Windows durability. macOS supplies local Unix filesystem evidence
only for installing Linux payloads. A support claim remains blocked until the
privileged unpacked Linux provisioner gate and separately specified native
macOS/Windows boundaries pass.
