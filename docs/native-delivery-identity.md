# Offline native delivery identity

`nextnc-native identity` returns JSON with schema `nextnc/compiler-identity/v1`,
`compiler_sha256`, `schema_sha256`, `policy_sha256` and `executable:false`.
The existing local diagnostic archive is reported separately. No source file,
controller connection, loaded policy or motion permission is needed.

The additive C export `nextnc_task_compiler_identity(uint8_t *, uint64_t)` returns
the same three hashes from the compiler embedded in the task shared library. The
caller provides exactly 192 writable bytes. Success writes three consecutive
64-byte lowercase ASCII SHA-256 strings, without a terminating NUL; errors return
through the existing FFI error channel. Null or any other extent is rejected
before writing. This probe belongs in offline packaging, never the task/servo loop.

The controller's native build tool compares the executable probe to the actual
shared-library probe and checks `nextnc_task_abi()`. A missing export or a different
compiler identity rejects a mixed delivery. The compiler identity includes the
existing build inputs/toolchain/target/profile policy; different build profiles
need not produce the same identity even when source commits match.

This adds an export without changing any existing ABI-5 structure or function.
The controller's pinned header/generated declarations must be updated together.
Binary hashes still identify the entire task library and host; the embedded
compiler identity alone does not identify all lowering/ownership implementation.
Neither identity equality nor compatible ABI proves Stage 5, real-time behavior,
machine readiness, complete execution, or physical acceptance.
