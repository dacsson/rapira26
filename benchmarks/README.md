# Benchmarks

Here we benchmark Rapira both against itself and against Python. We do want to initially be better then python in performance.

The benchmark runner validates Rapira and Python output before collecting any
timings. It measures both source-to-execution and precompiled RBC execution.
The benchmark programs themselves live in `benchmarks/sources/`; their paths
and inputs are declared in `cases.toml`. Large inputs live in
`benchmarks/inputs/` and are shared by the Rapira and Python implementations
through a case's `stdin_file` field. Small inputs can remain inline as
`rapira_stdin` or `python_stdin`.

```bash
python3 benchmarks/bench.py verify
python3 benchmarks/bench.py run
python3 benchmarks/bench.py --case fibonacci baseline --name before-change
python3 benchmarks/bench.py --case fibonacci compare --baseline before-change
```

Use `--case NAME` or `--tag TAG` to select cases, and `--samples` and
`--warmups` to tune a run. `compile` writes temporary RBC files to
`benchmarks/.build`; result history, baselines, and optional `profile` data
stay in `benchmarks/.results`. Both directories are intentionally ignored by
Git.

Set `RAPIRA_BIN` to benchmark another release binary. By default the runner
builds and uses `target/release/рапик`; use `--no-build` when it is already
current.

## Current limitations (by language/VM itself)

- no argc and argv passing to `main`, so we can only read from stdin
- no matrix like access like `a[i][j]` (rejected by frontend)
- no map/key-value like structure
- `вывод` formatting options, like `%2.f` etc.
- no good string std library
- no file reading
