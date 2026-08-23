#!/usr/bin/env python3
"""Benchmark Python and Rapira implementations and profile Rapira runs."""

from __future__ import annotations

import os
import statistics
import subprocess
import sys
import time
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
BENCHMARKS = Path(__file__).resolve().parent
RAPIRA = Path(os.environ.get("RAPIRA_BIN", ROOT / "target/release/рапик"))
PERF = os.environ.get("PERF", "perf")
REPEAT = 5

CASES = {
    "mandelbrot": {
        "python": [sys.executable, str(BENCHMARKS / "мандельброт.py")],
        "rapira": [str(RAPIRA), str(BENCHMARKS / "мандельброт.рап"), "--запуск"],
        "input": "100\n",
    },
    "fibonacci": {
        "python": [sys.executable, str(BENCHMARKS / "int_fib_mod.py")],
        "rapira": [str(RAPIRA), str(BENCHMARKS / "int_fib_mod.рап"), "--запуск"],
        "input": None,
    },
    "binary-tree": {
        "python": [
            sys.executable,
            str(BENCHMARKS / "binary_tree.py"),
            "18",
        ],
        "rapira": [str(RAPIRA), str(BENCHMARKS / "binary_tree.rap"), "--запуск"],
        "input": "18\n",
    },
}


def run(command: list[str], input_data: str | None = None) -> float:
    start = time.perf_counter()
    subprocess.run(
        command,
        cwd=ROOT,
        input=input_data,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        text=True,
        check=True,
    )
    return time.perf_counter() - start


def measure(command: list[str], input_data: str | None) -> float:
    # run(command, input_data)  # warm up
    samples = [run(command, input_data) for _ in range(REPEAT)]
    return statistics.median(samples)


def profile(case: str, command: list[str], input_data: str | None) -> Path:
    output = BENCHMARKS / f"perf-{case}.data"
    output.unlink(missing_ok=True)
    subprocess.run(
        [PERF, "record", "--call-graph", "dwarf", "-o", str(output), "--", *command],
        cwd=ROOT,
        input=input_data,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        text=True,
        check=True,
    )
    return output


def main() -> int:
    if not RAPIRA.exists():
        print(f"Rapira binary not found: {RAPIRA}", file=sys.stderr)
        print("Build it with: cargo build --release -p cli", file=sys.stderr)
        return 2

    print(f"Rapira: {RAPIRA}")
    print(f"Median of {REPEAT} runs; stdout suppressed; startup included")
    print(f"{'case':<14} {'python':>10} {'rapira':>10} {'diff':>10} {'rap/py':>10}")

    for case, commands in CASES.items():
        input_data = commands["input"]
        python_time = measure(commands["python"], input_data)
        rapira_time = measure(commands["rapira"], input_data)
        diff = rapira_time - python_time
        ratio = rapira_time / python_time
        print(
            f"{case:<14} {python_time:10.4f} {rapira_time:10.4f} "
            f"{diff:10.4f} {ratio:10.2f}"
        )

        perf_file = profile(case, commands["rapira"], input_data)
        print(f"  profile: {perf_file}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
