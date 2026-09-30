# Native bundles and publication

This is Stage 2 offline preparation infrastructure. It implements no motion
transport, controller selection/arm operation, start permission or resume cursor.
The native task owner in Stage 3 must integrate this lifecycle and negotiate fresh
controller state. Ordinary G-code and the existing JavaScript translator are
unchanged.

## Exact identities and independent decoding

The `.nncb` format is specified in [bundle-schema.txt](../src-rust/bundle-schema.txt).
It uses explicit little-endian fields, binary64 values and fixed versioned tags;
Rust enum layout, pointer width and struct padding are never serialized. It embeds
the exact source, reviewed setup and optional tool-table/target snapshot bytes,
followed by typed commands and their source-use spans. There is no cached live
coordinate state, controller epoch, execution cursor or run permission.

The artifact name is the SHA-256 of every byte. Its header binds compiler, schema
and policy hashes; its identity separately exposes exact source/setup/snapshot
hashes. `build.rs` derives compiler identity from implementation/shared-contract
sources, Cargo manifests/lock, the pinned toolchain, actual `rustc -vV`, target,
build profile, flags and features. Compiler text line endings are normalized to
LF for this build identity; job bytes are never normalized. Debug/release and
different target builds have different identities. A changed compiler/schema/
policy requires re-preparation; no unreviewed compatibility fallback is inferred.

Bundles are deterministic for identical exact inputs and compiler identity. A
timestamp, source pathname or output directory cannot affect their bytes. Jobs
with the same basename cannot collide unless their complete bundle bytes match.
Different whitespace in the source/setup changes content identity even when the
semantic fingerprint remains unchanged. Optional snapshot absence is distinct
from presence. The current baseline policy remains exact path with zero extra
fit/blend allowance and no geometric reductions; future policies must receive
distinct identities.

Encoding happens only after complete preparation. Its original in-memory plan
is then dropped, and a separately implemented reader reconstructs the actual
serialized plan. The reader validates embedded source/setup, command structure,
continuous geometry, ordered completeness, process policy and optional snapshots
before producing an immutable artifact. Unknown versions/tags, nonfinite values,
truncation, excessive counts and trailing bytes fail. A well-formed encoding with
a freshly recomputed checksum still fails if its commands disagree with source
or policy. The default bundle byte bound is 256 MiB, separate from the 32 MiB
source bound; neither is a measured job-capacity or real-time guarantee. Counts
are checked against available bytes and configured bounds before allocation.

## Owner and worker lifecycle

`publication::Store` holds an exclusive OS file lock for its lifetime. A lock
file alone is never treated as a live owner. Only an empty directory or a marked
native store is accepted; unrelated files and overlapping preparation inputs
are rejected before journal replacement. Immutable object names accept only
lowercase SHA-256 digests. Source/setup paths are fixed at request time and exact
bytes are rechecked during publication and whenever selection is queried.

1. `begin` clears the current selection and pending worker **before any fallible
   source or storage operation**. It creates a new owner/request generation and
   captures bounded, stable input bytes. Source/setup must be explicit; there is
   no global-plan or basename fallback.
2. The worker compiles those immutable snapshots and returns its opaque ticket
   and artifact. A worker cannot publish or select directly.
3. `commit` accepts only the current ticket from this owner. It checks exact
   snapshot identity, writes a private exclusive stage, flushes it, verifies its
   hash and independent semantic decode, then publishes a complete content-named
   object. Existing objects are verified before reuse. Source bytes are rechecked
   after staging and before publishing the selection journal.
4. The owner selects only after journal publication succeeds. Any failure for
   that request leaves it disarmed, even if a complete immutable object was
   already published. An obsolete worker result/cancellation cannot replace or
   disarm a newer request. Cancellation works before and after commit.
5. Querying a selection rechecks the exact inputs and stored artifact. Mutation,
   corruption, missing inputs or read failures invalidate the selection. Cached
   candidates associated with project paths need an explicit fresh `begin` and
   `select_existing` operation. To choose the immutable bundle itself as the job,
   `begin_bundle` independently validates its embedded inputs, creates a fresh
   generation and binds the exact bundle hash. Original project files and the
   original compiler process need not exist; the old selection is never restored.
   The returned candidate must still be committed by this owner. Corruption or
   replacement of that chosen bundle invalidates selection.

The disk journal is for inspection only. Its apparent status is never loaded as
permission or as an active selection. A new owner always starts disarmed with a
new epoch, even if the journal says `selected`, is incomplete, or could not be
updated when storage filled. A completed artifact remains independently usable
after the compiler exits, but requires fresh verification/selection and later
live admission. Private orphan stages after a crash are never candidates; they
are not automatically interpreted, selected or promoted.

Worker tickets are process-local opaque Rust values. The task owner may give
them to preparation threads; persisting/serializing a ticket cannot create a new
owner's authority. This checkpoint does not define an external-worker protocol.

## Failure evidence and limits

Tests cover repeated deterministic publication/reuse, stale workers, cancellation,
failed replacement, changed source/setup/tool tables, corrupted objects, invalid
object paths, input/store overlap, unrelated directories and exclusive ownership.
They also delete original project files and explicitly reselect a published job
under a new owner, without restoring the prior generation or any run permission.
Actual subprocess exits bypass destructors at six publication checkpoints;
reopening releases the dead owner's lock, restores no selection, and allows a
fresh explicit request. Partial stage/journal writes and storage failures are
injected through test-only checkpoints; they are not a claim that a physical
disk was filled. The normal executable has no job-controlled fault switch.

Files are flushed before rename; directory metadata is also synced on Unix.
Windows uses file flush and rename, without a POSIX directory-fsync claim.
Arbitrary power-loss behavior of a filesystem/device is not established by
process-exit tests. Restart disarming and full hash/semantic verification protect
selection even when files or journal entries are lost, truncated or reordered.
Stage 3 must still test the actual task/start interlock, live epochs, stop/abort
and controller recovery. This store is not a hostile-user security boundary or
a substitute for operating-system access controls.

The CLI's `publish` and `verify-bundle` tests clear PATH/environment and run with
no Node. Failed replacement produces no success JSON and leaves the prior object
available only for inspection. The command corpus still includes all four
complete legacy fixtures and all 130 native profiles, now passed through the
binary encoder and independent reader as well.
