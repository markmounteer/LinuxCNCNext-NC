# Shared Next-NC motion contract

This dependency-free `no_std` crate is the single source of the native semantic
vocabulary. It contains no transport, controller access, machine configuration or
execution adapter. Revision 1 remains at the root; revision 2 extends analytic
geometry, tolerance provenance and movement purpose in `v2`.

The Rust compiler consumes this workspace crate. The controller imports this
package by an exact Git commit and re-exports it through its workspace facade;
it does not maintain another copy of the type definitions. A compiler checkout
therefore requires no access to the private controller repository.

The initial source and tests were written for this Next-NC implementation and
moved from controller revision
`2c43deb48261661a243682f7bc7723ffc12a819c`. Only package/lint ownership and a
documentation pointer changed; type definitions and tests are unchanged.
No new capabilities are installed or negotiated by moving the source.

`cargo test -p motion-command --locked` runs revision and structural checks.
Numerical geometry, source semantics, live binding, limits, task ownership and
completion must be validated separately before any controller admission.
