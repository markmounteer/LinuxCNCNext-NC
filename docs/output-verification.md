# Output verification (v0.9.0)

These checks apply to the XZ lathe and fixed-axis XYZ mill profiles, in mm and
inch. They add no machining settings and do not change accepted profiles,
execution plans, fingerprints or numerical G-code output.

## Automatic final-text audit

Translation validates source geometry and the reviewed plan, creates typed
commands, and checks ordered source/plan consumption. It then constructs the
final G-code string and independently decodes that string against the commands.
Only after this audit succeeds does it hash and return the string for publication.
`preflight`, `translate` and the LinuxCNC filter all use this path; there is no
skip option.

The decoder recognizes only the translator's own dialect. It checks exact
finite numbers, initialization/reset modes, tool/H/WCS mappings, G53/work frames,
linear/rapid motion, planes and arc sense/centres/full circles, spindle/feed modes,
CSS limits, spindle selection, coolant, dwell and final M2. It rejects missing,
additional or conflicting words, exponent/comma numeric notation, extra blocks
and unexpected executable text. Signed zero keeps the existing convention.
Lathe X stays radius-based; mill Y and all three principal arc planes are retained.

Comments must match the ASCII sanitization/truncation contract and retain a
translator-owned passive prefix. This avoids maintaining a partial list of
[LinuxCNC active comments](https://www.linuxcnc.org/docs/html/gcode/overview.html#_comments),
whose syntax can trigger actions. User names retain their `Program:` or `Section`
prefix. The audit does not import the formatter or its numeric/comment helpers.

Success adds `inspection.execution.serialization` with schema
`linuxcnc-next-nc/serialization-audit/1`, status `passed`, checked-line,
executable-block and comment counts, and its scope. This is separate from the
ordered `execution.completeness` check. Older archives show “Not recorded”.

Failures use `INTERNAL_ERROR` and a `SERIALIZATION_*` invariant with the output
line, expected command, observed block/words and available operation/STEP/plan
context. They publish no G-code or successful preflight JSON. Existing diagnostic
archiving and exclusive file creation still apply. These are translator faults;
unrelated Fusion settings should not be changed to suppress them.

This is not an arbitrary RS274 interpreter, collision checker, machine-state
measurement or replacement for the standalone LinuxCNC tests and physical setup
validation. It compares text with validated records; it does not authenticate
those records against a maliciously modified translator.

## Check a saved G-code file when reviewing an archive

```sh
node bin/nextnc.js report diagnostic.json --gcode job.ngc --output NEW_REPORT.html
```

The optional `--gcode` argument is accepted only by `report`. It compares exact
bytes with `inspection.traceability.gcodeSHA256`, not the archive's top-level
`outputSHA256`. For a preflight archive the latter identifies the JSON output;
the traceability hash identifies the G-code candidate that was validated.

| Report result | Exit status | Meaning |
| --- | --- | --- |
| Match | 0 | The bytes read match the archived candidate hash. |
| Mismatch | 1 | A report is written, but its line map describes the archived candidate rather than the selected file. |
| Not recorded / Invalid recorded hash | 1 | A report is written without claiming a match; no fallback to another hash or filename. |
| Not checked | 0 | No file was supplied; existing archive-only review behavior. |

Unreadable/non-regular/oversized inputs or detectable changes during reading fail
without publishing a report. Incorrect CLI usage exits 2. Existing HTML
destinations are refused, including for mismatch reports.

The file limit is **64 MiB**, enforced on both handle metadata and actual reads.
Hashing uses one opened handle and bounded chunks. No line-ending, BOM, whitespace
or comment normalization occurs; even an otherwise equivalent text edit changes
identity. The derived HTML records file path, byte count, check time and both
hashes. It leaves the G-code, original archive and diagnostic indexes unchanged.

The tool opens only the explicitly supplied file; it never follows a stored
archive path automatically. The current AXIS helper is unchanged. A match is
relative to the archive, not a signature or proof of authenticity, current
controller loading, subsequent unchanged disk contents or successful execution.
This check does not parse or run the selected file.

See [validation](validation.md) for regression and interpreter evidence, and
[the upstream review](duy-translator-review-plan.md) for the original motivation.
