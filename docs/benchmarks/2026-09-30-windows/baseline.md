# Native compiler benchmark results

Compiler source: `40e15ad00363925402f423221ba4cd4b6704e614`. Worker SHA-256: `dc3568d01d7944106b936f9b73e94a67c005bc8fe6ad68110e64bf72b8ce7e02`.
Environment: Windows-11-10.0.26340-SP0; Intel64 Family 6 Model 198 Stepping 2, GenuineIntel.

Each case/phase uses 3 fresh processes and 6 calls per process. First call means a fresh process, with OS page caches uncontrolled. Warm values are medians of the remaining calls.

## Measurements

| Case | Prepare first/warm (ms) | Bundle compile first/warm (ms) | Validated reload first/warm (ms) | Peak RSS prepare/compile/load (MiB) |
| --- | ---: | ---: | ---: | ---: |
| mill-mm-arcs-1000 | 104.08 / 102.05 | 218.81 / 213.01 | 102.55 / 102.14 | 69.6 / 70.5 / 70.9 |
| lathe-mm-arcs-1000 | 110.89 / 107.02 | 222.20 / 217.18 | 101.16 / 100.36 | 71.1 / 71.9 / 72.7 |
| mill-mm-arcs-10000 | 1113.40 / 1119.68 | 2260.76 / 2234.54 | 1062.90 / 1057.90 | 627.7 / 637.9 / 646.1 |
| lathe-mm-arcs-10000 | 1091.54 / 1094.75 | 2305.18 / 2306.95 | 1124.05 / 1154.94 | 646.5 / 662.8 / 665.2 |
| mill-mm-arcs-34000 | 3872.90 / 4034.52 | 8299.23 / 8330.47 | 3833.57 / 3857.69 | 2129.3 / 2167.2 / 2192.9 |
| lathe-mm-arcs-34000 | 3980.38 / 4075.48 | 8738.12 / 8725.40 | 3908.87 / 4147.70 | 2196.0 / 2248.1 / 2260.8 |
| mill-inch-arcs-1000 | 98.02 / 95.50 | 208.61 / 203.25 | 96.43 / 96.34 | 69.5 / 70.4 / 70.9 |
| mill-mm-polyline-34000 | 137.67 / 136.05 | 287.43 / 281.32 | 136.00 / 133.48 | 97.8 / 102.5 / 106.9 |
| lathe-inch-arcs-1000 | 100.33 / 100.41 | 213.80 / 220.11 | 107.57 / 106.66 | 71.0 / 72.1 / 72.7 |
| lathe-mm-polyline-34000 | 142.82 / 139.23 | 290.12 / 286.90 | 138.00 / 138.04 | 98.1 / 103.3 / 106.8 |

## Interpretation

Prepare includes complete source/setup parsing and independent command audits. Bundle compile repeats prepare and includes independent reload; columns overlap and must not be added. Reload revalidates embedded inputs, identities and every serialized command. It restores no live binding or execution permission.

Core timing excludes input reads, buffer copies, result checking/destruction and diagnostic output. OS peak RSS includes those allocations and warm-process allocator retention. The raw results retain every sample, per-process memory counter, artifact identity and exact command/span digest. No controller, step generator, machining-time or Raspberry Pi measurement is represented.
