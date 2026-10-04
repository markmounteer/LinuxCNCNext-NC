# Native compiler benchmark results

Compiler source: `9a518ab2f12b15319ad593c205ed199c8d267d97`. Worker SHA-256: `c76c6ec0703e9445f5047ea22c195089cb86851cd9800cc4a3519b356c84c77c`.
Environment: Windows-11-10.0.26340-SP0; Intel64 Family 6 Model 198 Stepping 2, GenuineIntel.

Each case/phase uses 3 fresh processes and 6 calls per process. First call means a fresh process, with OS page caches uncontrolled. Warm values are medians of the remaining calls.

## Measurements

| Case | Prepare first/warm (ms) | Bundle compile first/warm (ms) | Validated reload first/warm (ms) | Peak RSS prepare/compile/load (MiB) |
| --- | ---: | ---: | ---: | ---: |
| mill-mm-arcs-1000 | 73.12 / 71.92 | 151.83 / 152.79 | 73.64 / 72.22 | 45.9 / 47.6 / 47.2 |
| lathe-mm-arcs-1000 | 76.36 / 75.72 | 165.20 / 167.71 | 78.23 / 75.85 | 46.6 / 48.8 / 48.5 |
| mill-mm-arcs-10000 | 812.13 / 827.75 | 1728.52 / 1833.89 | 846.62 / 841.76 | 394.3 / 412.0 / 412.5 |
| lathe-mm-arcs-10000 | 835.03 / 852.14 | 1788.18 / 1838.18 | 835.16 / 826.10 | 405.9 / 424.1 / 424.5 |
| mill-mm-arcs-34000 | 2821.33 / 2944.08 | 6146.71 / 6214.04 | 2738.83 / 2795.22 | 1334.2 / 1397.2 / 1398.1 |
| lathe-mm-arcs-34000 | 2865.36 / 2987.42 | 6455.89 / 6433.18 | 2881.98 / 2992.65 | 1375.8 / 1439.7 / 1440.3 |
| mill-inch-arcs-1000 | 71.05 / 72.07 | 153.53 / 152.32 | 73.59 / 71.73 | 45.4 / 47.7 / 47.4 |
| mill-mm-polyline-34000 | 113.42 / 111.98 | 246.13 / 228.33 | 103.79 / 106.02 | 73.9 / 78.9 / 82.7 |
| lathe-inch-arcs-1000 | 74.77 / 72.66 | 155.11 / 153.40 | 74.10 / 73.32 | 46.4 / 48.2 / 48.2 |
| lathe-mm-polyline-34000 | 112.32 / 111.89 | 235.05 / 228.86 | 108.39 / 108.79 | 73.8 / 78.2 / 83.1 |

## Matched baseline comparison

Baseline source: `40e15ad00363925402f423221ba4cd4b6704e614`. Inputs, environment, schema/policy and complete ordered commands/spans agree. Values below are candidate/baseline ratios; below 1 is a measured reduction. These repeated workstation measurements do not establish causality for small timing differences or real-time deadlines.

| Case | Prepare warm ratio | Compile warm ratio | Reload warm ratio | Peak RSS ratio prepare/compile/load |
| --- | ---: | ---: | ---: | ---: |
| mill-mm-arcs-1000 | 0.705 | 0.717 | 0.707 | 0.659 / 0.675 / 0.666 |
| lathe-mm-arcs-1000 | 0.708 | 0.772 | 0.756 | 0.655 / 0.678 / 0.668 |
| mill-mm-arcs-10000 | 0.739 | 0.821 | 0.796 | 0.628 / 0.646 / 0.638 |
| lathe-mm-arcs-10000 | 0.778 | 0.797 | 0.715 | 0.628 / 0.640 / 0.638 |
| mill-mm-arcs-34000 | 0.730 | 0.746 | 0.725 | 0.627 / 0.645 / 0.638 |
| lathe-mm-arcs-34000 | 0.733 | 0.737 | 0.722 | 0.626 / 0.640 / 0.637 |
| mill-inch-arcs-1000 | 0.755 | 0.749 | 0.745 | 0.654 / 0.678 / 0.668 |
| mill-mm-polyline-34000 | 0.823 | 0.812 | 0.794 | 0.755 / 0.770 / 0.774 |
| lathe-inch-arcs-1000 | 0.724 | 0.697 | 0.687 | 0.653 / 0.669 / 0.664 |
| lathe-mm-polyline-34000 | 0.804 | 0.798 | 0.788 | 0.753 / 0.758 / 0.778 |

## Interpretation

Prepare includes complete source/setup parsing and independent command audits. Bundle compile repeats prepare and includes independent reload; columns overlap and must not be added. Reload revalidates embedded inputs, identities and every serialized command. It restores no live binding or execution permission.

Core timing excludes input reads, buffer copies, result checking/destruction and diagnostic output. OS peak RSS includes those allocations and warm-process allocator retention. The raw results retain every sample, per-process memory counter, artifact identity and exact command/span digest. No controller, step generator, machining-time or Raspberry Pi measurement is represented.
