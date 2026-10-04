# Original source-partition counterexample

These four input files preserve the exact bytes from the independent Stage 5
audit on 2026-10-03, under `repro` in the frozen `nextnc-20261003-independent`
audit. They describe the same three physical cutting segments at 2 mm/s; the
split source restarts its polyline ordinal before the last segment. Source names
and the corresponding setup fingerprint differ; geometry and process intent do
not. These synthetic jobs are regression inputs, not machine-approved programs.

| Input | SHA-256 |
| --- | --- |
| joined.stpnc | ae2a0985ca20209af12a94a2802d8d8107762bbc3d2334c8e6f94a87be4c90b4 |
| joined.plan.json | 3f5bf4498cc8e435fdfeae8b1412a0338028b56ca7b7e51d8a13e3fb5cb3931b |
| split.stpnc | 3880ccf6b2121e55633041faee844d5a5554423ad45fa9c98fee9b6eedddfb2f |
| split.plan.json | e8cad08064a49af48d685763bbbb810ec389da76879efa28fc68ecda3bb14ffe |

The public integration test prepares, binds and lowers each complete input.
It requires identical actions, geometry, feeds, limits, command ranges and drain
flags, with one three-command corner budget, while retaining source ordinals
`[1, 2, 3]` and `[1, 2, 1]`. Running this test against the preceding grouping
implementation fails because the split input receives no corner budget.
