# Current performance state

## Results

3-warmup, 5-sample method:

| Benchmark          |   Python | Rapira26 E2E | Rapira26 VM-only | E2E/Python | VM/Python | E2E improvement | VM improvement |
| ------------------ | -------: | -----------: | ---------------: | ---------: | --------: | --------------: | -------------: |
| mandelbrot         | 1.0824 s |     1.8227 s |         1.8169 s |       1.68 |      1.68 |           56.5% |          56.7% |
| fibonacci          | 6.1225 s |     2.6090 s |         2.6077 s |       0.43 |      0.43 |           45.8% |          44.0% |
| binary-tree        | 4.2234 s |    10.5760 s |        10.5312 s |       2.50 |      2.49 |           14.7% |          15.0% |
| nbody              | 1.4187 s |     3.2440 s |         3.2449 s |       2.29 |      2.29 |           37.9% |          38.0% |
| fannkuch-redux     | 4.6494 s |     9.7255 s |         9.7724 s |       2.09 |      2.10 |           11.2% |          11.2% |
| reverse-complement | 0.0255 s |     1.5384 s |         1.5721 s |      60.22 |     61.54 |           21.5% |          11.0% |

## Version

- Run timestamp: `2026-09-13T17:38:07.882846+00:00`
- Git commit: `d9a24ab736a0aef1e92758cf2ea949f42d2200ca`
- Commit message: `[VM] Fast path for zero fields variants`
- Commit date: `2026-09-13T20:25:39+03:00`

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
