# Current performance state

## Results

3-warmup, 5-sample method:

| Benchmark | Python | Rapira26 E2E | Rapira26 VM-only | E2E/Python | VM/Python |
|---|---:|---:|---:|---:|---:|
| mandelbrot | 1.0844 s | 5.3999 s | 5.4126 s | 4.98 | 4.99 |
| fibonacci | 5.9731 s | 5.2093 s | 5.2340 s | 0.87 | 0.88 |
| binary-tree | 4.0844 s | 12.7725 s | 12.7674 s | 3.13 | 3.13 |
| nbody | 1.4607 s | 5.4698 s | 5.4725 s | 3.74 | 3.75 |
| fannkuch-redux | 4.6351 s | 11.5986 s | 11.5811 s | 2.50 | 2.50 |
| reverse-complement | 0.0266 s | 1.9473 s | 2.0369 s | 73.29 | 76.67 |

## Version

- Run timestamp: `2026-09-05T15:46:02.151105+00:00`
- Reverse-complement run timestamp: `2026-09-05T16:14:25.455483+00:00`
- Git commit: `2a6e7a79ab3b89eae049b39d2b4337386a392eeb`
- Commit message: `[doc] Separate modules description, mention (dare i say to our decrement) current performance numbers`
- Commit date: `2026-09-04T23:43:56+03:00`

## Environment

- CPU: `12th Gen Intel(R) Core(TM) i5-1235U`
- Architecture: `x86_64`
- Operating system: `Linux`
- Rapira executable: release build

## Method

Command:

```bash
python3 -B benchmarks/bench.py run --samples 5
```

The refreshed reverse-complement row used:

```bash
python3 -B benchmarks/bench.py --no-build --case reverse-complement run --samples 5
```

Each result is the median of 5 measured executions after 3 warmups. Before
timing, the runner validated Python, Rapira source execution, and precompiled
RBC output against the SHA-256 values in `benchmarks/cases.toml`.

- Python includes interpreter and process startup.
- End-to-end includes Rapira process startup, parsing, bytecode generation,
  bytefile loading, and VM execution.
- VM-only executes a precompiled RBC file and excludes source compilation.
- `e2e/py` and `vm/py` are ratios; smaller is better.
