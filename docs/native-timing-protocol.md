# Native timing binding (task ABI 3)

Historical ABI-3 checkpoint. Current ABI 4 retains this timing record and adds
[observed trajectory limits and numerical headroom](native-numerical-budget.md).
The snapshot size and test counts below describe the earlier binding.

The live snapshot appends `nextnc_timing_evidence`: model, nominal servo interval,
trajectory interval, interpolation ratio, cubic segment interval, motion instance
and four motion-birth words. Intervals use integer nanoseconds. The record is
40 bytes; the complete snapshot is 1,216 bytes with timing at offset 1,176.

Model 1 requires positive equal intervals and ratio one. The host supplies actual
motion/interpolator observations, not a parsed INI setting or a job declaration.
The decoder refuses missing, inconsistent or unsupported evidence. Lowering now
retains the observed interval for subsequent corner-budget derivation, with no
default value. This checkpoint does not implement that derivation.

The complete timing record enters the configuration/environment fingerprints and
is also retained directly on the immutable candidate. Tool confirmation and
suffix rebind may accommodate reviewed tool-table changes, but cannot substitute
a new period or motion instance. Failure returns a zero output and preserves the
prior candidate. Old ABI headers are checked before any extended body read.

Windows task/FFI tests (53) and Linux compiler-workspace tests (145) pass, along
with Clippy. Tests cover several explicit synthetic intervals, malformed fields,
old header-only inputs, changes to timing, instance and each birth word, and a
real tool-procedure suffix rebind with changed tool offsets. Controller-side
evidence is recorded in the companion LinuxCNC motion workspace under
`docs/nextnc-native-stage5-observed-timing.md`: 52 NML representation cases,
eight complete mill/lathe native/reference simulator jobs at 1/2 ms, and four
missing-observer refusal/ordinary-execution cases. Its four failed 0.5 ms startups
remain retained failures, not supported runtime configurations.

Stage 5 remains open. These checks establish nominal timing identity; they do
not establish continuous comparative path accuracy, deadlines or hardware
acceptance. No controller was contacted or modified.
