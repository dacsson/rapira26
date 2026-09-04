# Current performance state

## Results

3-warmup, 15-sample method:

| Benchmark | Python | End-to-end | VM-only | E2E/Python | VM/Python |
|---|---:|---:|---:|---:|---:|
| mandelbrot | 0.3061 s | 1.5720 s | 1.5746 s | 5.14 | 5.14 |
| fibonacci | 0.4418 s | 0.3947 s | 0.3922 s | 0.89 | 0.89 |
| binary-tree | 4.1143 s | 14.1997 s | 14.7033 s | 3.45 | 3.57 |
| nbody | 0.4165 s | 1.4237 s | 1.4669 s | 3.42 | 3.52 |
| fannkuch-redux | 0.4201 s | 1.0048 s | 1.0112 s | 2.39 | 2.41 |
| reverse-complement | 0.0244 s | 2.4052 s | 2.4070 s | 98.63 | 98.70 |

## Version

- Run timestamp: `2026-09-04T20:13:04.603086+00:00`
- Git commit: `a833cf0c9d184a3424fbecb50b97d4a80727673d`
- Commit message: `[org] Mention demos from playground`
- Commit date: `2026-08-27T16:42:55+03:00`

## Environment

- CPU: `12th Gen Intel(R) Core(TM) i5-1235U`
- Architecture: `x86_64`
- Operating system: `Linux`
- Rapira executable: release build

## Method

Command:

```bash
python3 -B benchmarks/bench.py run
```

Each result is the median of 15 measured executions after 3 warmups. Before
timing, the runner validated Python, Rapira source execution, and precompiled
RBC output against the SHA-256 values in `benchmarks/cases.toml`.

- Python includes interpreter and process startup.
- End-to-end includes Rapira process startup, parsing, bytecode generation,
  bytefile loading, and VM execution.
- VM-only executes a precompiled RBC file and excludes source compilation.
- `e2e/py` and `vm/py` are ratios; smaller is better.
