# Current performance state

## Results

3-warmup, 5-sample method:

| Benchmark | Python | Rapira26 E2E | Rapira26 VM-only | E2E/Python | VM/Python |
|---|---:|---:|---:|---:|---:|
| mandelbrot | 1.0793 s | 4.1947 s | 4.1937 s | 3.89 | 3.89 |
| fibonacci | 6.2561 s | 4.8126 s | 4.6597 s | 0.77 | 0.74 |
| binary-tree | 4.0518 s | 12.4003 s | 12.3934 s | 3.06 | 3.06 |
| nbody | 1.4144 s | 5.2242 s | 5.2329 s | 3.69 | 3.70 |
| fannkuch-redux | 4.6320 s | 10.9484 s | 11.0012 s | 2.36 | 2.38 |
| reverse-complement | 0.0241 s | 1.9590 s | 1.7655 s | 81.20 | 73.17 |

## Version

- Run timestamp: `2026-09-10T14:53-44.612980+00-00`
- Git commit: `cfcac89f24d9d349731156bfdbb4dc488e30a6f8`
- Commit message: `[runtime][perf][VM] Add hints for branch predictor, fix text repeat...`
- Commit date: `2026-09-10T18:22:05+03:00`

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

Each result is the median of 5 measured executions after 3 warmups. Before
timing, the runner validated Python, Rapira source execution, and precompiled
RBC output against the SHA-256 values in `benchmarks/cases.toml`.

- Python includes interpreter and process startup.
- End-to-end includes Rapira process startup, parsing, bytecode generation,
  bytefile loading, and VM execution.
- VM-only executes a precompiled RBC file and excludes source compilation.
- `e2e/py` and `vm/py` are ratios; smaller is better.
