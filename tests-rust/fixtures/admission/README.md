# Measured reference admission fixture

`reference.json` is the unchanged summary of 21 passing synthetic reference cases
from controller harness `9b2df9144c8d6662af0756548df138dc8868ed03`.
Its SHA-256 is `f5ce23faeb6cf46d262091c8a531e14b9b9124910884d607f20ecdf4312edd5c`.
It contains generated XYZ/XZ paths and simulator identities, not a user's job.

[Full qualification, raw traces and manifests](https://github.com/markmounteer/linuxcnc/blob/4a994d56012b7f024ee44015960ea39003b6dcba/controller/motion/motion/tests/nextnc-stage0/evidence/2026-09-30-admission/README.md)
record the custom source, reconstructed tree, image/component hashes, failed
exploratory matrices, actual completion, all trace audits and the same image's
28-case baseline regression. The source repository may require access; the exact
summary used by the public compiler tests is included here for reproducibility.

These are x86-64 WSL/container observations with input shaping disabled. They
are not native-executor capacity, Raspberry Pi timing or physical acceptance.
The Rust reader validates the complete summary and arithmetic; raw observations
were independently checked by the recorded harness. The fixture establishes
neither trust in arbitrary replacement reports nor permission to run a machine.
