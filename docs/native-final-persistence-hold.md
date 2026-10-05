# Hold during final native reconciliation

A LinuxCNC simulator test on runtime `76eb08fa` reproduced a rejected Hold while
the third, final parameter save was pending: `native task lifecycle: State`.
The task was stationary, its lease remained armed, and final reconciliation had
not completed. `Owner::hold` excluded `Reconciling`.

The lifecycle now allows this hold and remembers `Reconciling` as the resume
phase. Resume returns to reconciliation without reopening command admission or
changing the completed prefix. The lifecycle test covers repeated hold, effective
hold acknowledgement, refused step, stale/disabled resume, withheld MDI/release,
normal resume/completion, and abort from the held barrier. It reproduced the
original failure before the implementation change.

The companion LinuxCNC task-host change rejects new hold/resume/step controls once
lease release has already been requested. That is a different boundary from a
pending durable save. Abort retains its separate path. The guard has a component
test; a deterministic integrated post-durable/pre-release race test remains open.

Windows and Linux task/FFI suites pass 88 tests, with formatting and strict Clippy.
The Linux Rust host passes 43 tests plus its format oracle and release build.
The integrated candidate image
`sha256:0e880d37df368863cb3ba490d66afa450f324d1838083ca203024154bd1f7511`
passes eight stationary pending-save cases: abort, hold/resume, task death with
replacement refusal, and shutdown, at both first and final saves. The original
final-save hold failure remains captured. The engine library SHA-256 is
`44a2e7de68c1afea2d80af94dcc4affeecf58da7a2443726685e75f3201c9dd4`.

The LinuxCNC repository retains the build and simulator evidence under
`controller/motion/motion/tests/nextnc-stage5/evidence/persistence-final-hold-r1`.
These are finite stationary-barrier controls, not fresh-owner recovery, moving
stopping proofs, continuous-path certification or WCET. Stage 5 remains incomplete;
no controller contact, deployment or physical acceptance was performed.
