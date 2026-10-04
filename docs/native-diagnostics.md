# Native preparation diagnostics

Every `nextnc-native` command attempts to save a JSON report and a readable TXT
report before returning its result. Reports describe offline preparation only.
They contain no execution permission, live coordinate binding or machine-ready
claim. The Rust executable needs no Node runtime or external collector.

## Standard location

| Platform | Default directory |
| --- | --- |
| Windows | `%LOCALAPPDATA%\LinuxCNCNext-NC\diagnostics` |
| Windows fallback | `%USERPROFILE%\AppData\Local\LinuxCNCNext-NC\diagnostics` |
| Linux | `$XDG_STATE_HOME/LinuxCNCNext-NC/diagnostics` |
| Linux fallback | `$HOME/.local/state/LinuxCNCNext-NC/diagnostics` |

`NEXTNC_DIAGNOSTICS` overrides the directory. An explicit relative override is
resolved against the working directory. Default platform/home paths must be
absolute; an environment without either has no implicit working-directory
fallback. The returned JSON then describes archiving as unavailable. This is
the existing translator's directory convention, separate from Fusion's
`Fusion360Next-NC` post-engine collector directory.

Open `latest-error.txt` for the latest failed invocation. `latest-error.json`
contains the same structured evidence. A successful invocation updates
`latest.txt` and `latest.json` and preserves the latest error. Each invocation
also has unique `native-<time>-<pid>-<sequence>.json` and `.txt` files. The CLI's
`diagnostics` field gives their actual absolute paths, report ID, archive status
and whether all latest indexes were updated. No old reports are automatically
deleted.

## Recorded evidence

- Report schema/version, UTC time, elapsed preparation time, compiler identity,
  source-profile schema identity and policy identity.
- Command and intended input paths; exact byte count and SHA-256 only for inputs
  actually observed. Unread inputs have `observed:false` and no invented hash.
  Publication records its captured snapshot. If snapshot creation fails, it
  does not claim an exact identity for a partial read that was not returned.
- Actual CLI-stage results: `passed`, `failed`, `not_reached` or `not_checked`.
  Compile/bundle stages encompass their internal validators; a stage list is not
  a separate claim that every possible semantic check has been inventoried.
- The original error code/message/stage, source record and one-based UTF-8 byte
  location when available, section/path context, all collected plan issues and
  missing tool/capability uses. JSON syntax errors include their own one-based
  JSON line and byte column; they are not mislabeled as STEP-file locations.
- Correction guidance and up to seven source lines, bounded to 8 KiB. An excerpt
  is included only after rereading the named input and matching its exact
  observed SHA-256. Changed, unread, unavailable or incorrectly located inputs
  receive an explicit explanation instead. Embedded bundle source locations are
  never interpreted as line numbers in the outer binary artifact.
- A result summary; full decoded source models and command arrays are excluded.
  The structured report and readable report each have a 16 MiB serialization
  limit. Controls and bidirectional formatting characters are escaped in TXT.

`analyze-rate` records the independently checked artifact and admission-reference
identities, demand/expansion counts, observed workloads, assumptions and warnings.
Reference validation has separate stages. Malformed measurement JSON receives an
excerpt from that observed file; it is never attributed to embedded STEP source
or the binary bundle. No measured rate is cached as execution permission.

The JSON schema is `linuxcnc-next-nc/native-diagnostic/1`, distinct from the
retained JavaScript report schema. Byte columns also differ from legacy UTF-16
string columns. Consumers should check the schema before interpreting locations.

## Publication and failure behavior

Private files are created exclusively and flushed before publication. A hard
link publishes each unique immutable report without replacing another record;
this requires a filesystem supporting hard links, such as NTFS or ext4. A
filesystem that cannot publish a report returns an archive error. Temporary
files from interrupted writes are not promoted on restart.

A bounded OS file lock serializes native latest-index updates. Each alias is
replaced atomically; the JSON/TXT pair and latest/latest-error aliases are not a
single filesystem transaction. Use report IDs to detect a pair interrupted
between replacements. The older JavaScript writer does not participate in this
lock. A busy native index writer leaves complete unique reports and reports
`latestUpdated:false`. Unix additionally syncs the directory; Windows flushes
files before link/rename. These are process/filesystem semantics, not evidence
of hardware power-loss durability.

Known input paths are protected against index/lock collisions, including
argument-validation failures. Nonordinary directory/index/lock targets are
rejected. Reports are local to the chosen directory; nothing is transmitted.

Archiving is best effort and separate from preparation:

- Failure still returns exit status 1, no success stdout, and the original error
  on stderr, even if the diagnostics directory is unavailable or unwritable.
- Successful preparation retains its success status if archiving fails, with a
  separate `diagnostics.status` of `unavailable` or `partial`.
- A partial archive exposes any complete immutable report paths. It never
  changes a job's selection, permissions or source bytes.

`tests-rust/diagnostics.rs` covers directory resolution, observed identities,
changed sources, setup/tool-table excerpts, UTF-8/read/report limits, stage
states, UTC/control handling, concurrent writers, busy indexes, input collisions
and storage failures. `tests-rust/native_cli.rs` exercises all six commands with
no PATH/Node, success/error persistence, unread stages, correct input locations,
archive failures and argument-error collision protection. This complements the
parser/profile/plan/audit negative suites; it does not qualify physical motion.
