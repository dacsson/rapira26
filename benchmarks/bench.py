#!/usr/bin/env python3
"""Run, compare, and locally record Rapira benchmarks.

This script is the entry point for performance work in this repository.  It
deliberately keeps the measurement machinery in Python's standard library so
contributors can run it after a normal Rust build; no Python package install
is required.

Quick start
-----------

    # Verify that the current implementations still compute expected answers.
    python3 benchmarks/bench.py verify

    # Measure Python, source-to-VM Rapira, and precompiled-RBC Rapira.
    python3 benchmarks/bench.py run

    # Focus on a case or a workload category.
    python3 benchmarks/bench.py --case fibonacci run
    python3 benchmarks/bench.py --tag allocation run

    # Save a local reference before a change, then compare after it.
    python3 benchmarks/bench.py baseline --name before-refcount-change
    python3 benchmarks/bench.py compare --baseline before-refcount-change

What is measured
----------------

``run``, ``baseline``, and ``compare`` validate outputs first, then take three
sets of samples for every selected case:

* ``python`` runs the matching Python program.  It is context for the Rapira
  result, not a compatibility requirement for the VM.
* ``end-to-end`` invokes ``рапик source.rap``.  It includes process startup,
  parsing, module resolution, bytecode generation, bytefile parsing, and VM
  execution.
* ``vm-only`` first compiles the same source to ``.rbc``, then invokes
  ``рапик program.rbc``.  It removes source compilation work, which makes it
  useful for attributing regressions to the VM/runtime instead of the compiler.

The printed ``e2e/py`` and ``vm/py`` columns are Rapira/Python ratios: below
``1.00`` means that Rapira was faster for that particular workload.  Comparison
output appends three percentage deltas in the same order as the timing columns;
negative values are faster than the chosen baseline.

Cases and correctness hashes
-----------------------------

``benchmarks/cases.toml`` is the source of truth for the suite.  A ``[[case]]``
entry needs a stable ``name``, Rapira and Python paths, optional stdin and
Python arguments, output hashes, and tags.  Small inputs can use TOML strings,
so a trailing newline is written as ``"18\\n"``.  Larger shared inputs should
use ``stdin_file``; its path is relative to the repository root and its entire
UTF-8 contents are supplied to both implementations.  Language-specific
``rapira_stdin`` and ``python_stdin`` values override the shared file.

``output_sha256`` is the SHA-256 hash of complete stdout bytes, including final
newlines.  It is checked for both languages before timing so a faster but
incorrect program is never reported as an improvement.  When equivalent
results intentionally require different formatting, ``rapira_output_sha256``
or ``python_output_sha256`` can override the shared value for that language.

To add a case, add the two programs, run each at its intended input, calculate
the stdout hashes with ``sha256sum``, then add a manifest entry.  Choose inputs
that are deterministic and long enough to avoid startup noise dominating the
result.  Run ``--case NAME verify`` before trusting a measurement.

Generated directories
---------------------

The runner creates two ignored directories under ``benchmarks``:

* ``.build/`` contains generated ``NAME.rbc`` bytefiles.  ``compile`` creates
  them explicitly; timing commands refresh them after source validation.  They
  are disposable and must never be edited or committed.
* ``.results/MACHINE_ID/`` is local performance history.  ``MACHINE_ID`` is a
  short SHA-256-derived identifier based on CPU model, architecture, operating
  system, and kernel.  It prevents a baseline recorded on one computer from
  being silently compared against a different computer.

Each normal timing run writes one timestamped JSON file containing raw samples,
medians, Git revision/dirty status, runner settings, and machine metadata.
``baseline --name NAME`` additionally writes
``.results/MACHINE_ID/baselines/NAME.json``.  These paths are ignored by Git:
they are personal machine history, not project-wide authoritative results.

Operational notes
-----------------

The runner builds ``target/release/рапик`` by default.  Supply ``--no-build``
when it is already current, or set ``RAPIRA_BIN`` to measure another executable
(for example, one from a separate worktree).  Defaults are three warmups and
15 timed samples; use ``--warmups`` and ``--samples`` to adjust them.  Use
``profile`` with exactly one selected case to write an opt-in ``perf record``
capture into the local result directory; routine timing never invokes perf.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import statistics
import subprocess
import sys
import time
import tomllib
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
BENCHMARKS = Path(__file__).resolve().parent
MANIFEST = BENCHMARKS / "cases.toml"
BUILD_DIR = BENCHMARKS / ".build"
RESULTS_DIR = BENCHMARKS / ".results"
DEFAULT_SAMPLES = 15
DEFAULT_WARMUPS = 3


@dataclass(frozen=True)
class BenchmarkCase:
    name: str
    rapira_source: Path
    python_source: Path
    rapira_stdin: str | None
    python_stdin: str | None
    python_args: tuple[str, ...]
    rapira_output_sha256: str
    python_output_sha256: str
    tags: tuple[str, ...]


def load_cases(manifest: Path = MANIFEST) -> list[BenchmarkCase]:
    with manifest.open("rb") as file:
        data = tomllib.load(file)
    cases = []
    for item in data.get("case", []):
        shared_stdin = None
        if stdin_file := item.get("stdin_file"):
            shared_stdin = (ROOT / stdin_file).read_text()
        shared_output_sha256 = item.get("output_sha256")
        rapira_output_sha256 = item.get("rapira_output_sha256", shared_output_sha256)
        python_output_sha256 = item.get("python_output_sha256", shared_output_sha256)
        if rapira_output_sha256 is None or python_output_sha256 is None:
            raise ValueError(
                f"case {item['name']} needs output_sha256 or both language-specific output hashes"
            )
        cases.append(
            BenchmarkCase(
                name=item["name"],
                rapira_source=ROOT / item["rapira_source"],
                python_source=ROOT / item["python_source"],
                rapira_stdin=item.get("rapira_stdin", shared_stdin),
                python_stdin=item.get("python_stdin", shared_stdin),
                python_args=tuple(item.get("python_args", [])),
                rapira_output_sha256=rapira_output_sha256,
                python_output_sha256=python_output_sha256,
                tags=tuple(item.get("tags", [])),
            )
        )
    if not cases:
        raise ValueError(f"no benchmark cases in {manifest}")
    return cases


def select_cases(cases: list[BenchmarkCase], names: list[str], tags: list[str]) -> list[BenchmarkCase]:
    unknown_names = set(names) - {case.name for case in cases}
    if unknown_names:
        raise ValueError(f"unknown benchmark case(s): {', '.join(sorted(unknown_names))}")
    selected = [
        case for case in cases
        if (not names or case.name in names) and (not tags or set(tags).intersection(case.tags))
    ]
    if not selected:
        raise ValueError("no cases match the requested filters")
    return selected


def rapira_binary() -> Path:
    return Path(os.environ.get("RAPIRA_BIN", ROOT / "target/release/рапик"))


def command_output(command: list[str], stdin: str | None) -> bytes:
    completed = subprocess.run(command, cwd=ROOT, input=stdin, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True)
    if completed.returncode:
        raise RuntimeError(f"{' '.join(command)} exited with {completed.returncode}:\n{completed.stderr}")
    return completed.stdout.encode()


def validate_output(command: list[str], stdin: str | None, expected_hash: str, label: str) -> None:
    actual_hash = hashlib.sha256(command_output(command, stdin)).hexdigest()
    if actual_hash != expected_hash:
        raise RuntimeError(f"{label} output changed: expected {expected_hash}, got {actual_hash}")


def timed_samples(command: list[str], stdin: str | None, warmups: int, samples: int) -> list[int]:
    for _ in range(warmups):
        subprocess.run(command, cwd=ROOT, input=stdin, stdout=subprocess.DEVNULL,
                       stderr=subprocess.PIPE, text=True, check=True)
    measurements = []
    for _ in range(samples):
        started = time.perf_counter_ns()
        subprocess.run(command, cwd=ROOT, input=stdin, stdout=subprocess.DEVNULL,
                       stderr=subprocess.PIPE, text=True, check=True)
        measurements.append(time.perf_counter_ns() - started)
    return measurements


def summary(samples: list[int]) -> dict[str, int | list[int]]:
    return {"samples_ns": samples, "median_ns": int(statistics.median(samples)),
            "min_ns": min(samples), "max_ns": max(samples)}


def build_release(binary: Path, no_build: bool) -> None:
    if not no_build:
        subprocess.run(["cargo", "build", "--release", "-p", "cli"], cwd=ROOT, check=True)
    if not binary.exists():
        raise RuntimeError(f"Rapira binary not found: {binary}")


def source_command(binary: Path, case: BenchmarkCase) -> list[str]:
    return [str(binary), str(case.rapira_source), "--запуск"]


def python_command(case: BenchmarkCase) -> list[str]:
    return [sys.executable, str(case.python_source), *case.python_args]


def bytefile_path(case: BenchmarkCase) -> Path:
    return BUILD_DIR / f"{case.name}.rbc"


def compile_cases(binary: Path, cases: list[BenchmarkCase]) -> None:
    BUILD_DIR.mkdir(parents=True, exist_ok=True)
    for case in cases:
        subprocess.run([str(binary), str(case.rapira_source), "--сохранить-байткод",
                        str(bytefile_path(case))], cwd=ROOT, check=True)


def verify_cases(binary: Path, cases: list[BenchmarkCase], verify_bytecode: bool = False) -> None:
    for case in cases:
        validate_output(source_command(binary, case), case.rapira_stdin,
                        case.rapira_output_sha256, f"{case.name} Rapira")
        validate_output(python_command(case), case.python_stdin,
                        case.python_output_sha256, f"{case.name} Python")
        if verify_bytecode:
            validate_output([str(binary), str(bytefile_path(case))], case.rapira_stdin,
                            case.rapira_output_sha256, f"{case.name} RBC")


def git_value(args: list[str], fallback: str) -> str:
    completed = subprocess.run(args, cwd=ROOT, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, text=True)
    return completed.stdout.strip() if completed.returncode == 0 else fallback


def cpu_model() -> str:
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.partition(":")[2].strip()
    except OSError:
        pass
    return platform.processor() or "unknown"


def machine_metadata() -> dict[str, str]:
    return {"architecture": platform.machine(), "cpu": cpu_model(),
            "kernel": platform.release(), "system": platform.system()}


def machine_id(machine: dict[str, str]) -> str:
    """Return a stable, non-secret directory name for comparable local runs."""
    return hashlib.sha256(json.dumps(machine, sort_keys=True).encode()).hexdigest()[:12]


def collect_results(binary: Path, cases: list[BenchmarkCase], warmups: int, samples: int) -> dict[str, Any]:
    results = {}
    for case in cases:
        results[case.name] = {
            "python": summary(timed_samples(python_command(case), case.python_stdin, warmups, samples)),
            "end_to_end": summary(timed_samples(source_command(binary, case), case.rapira_stdin, warmups, samples)),
            "vm_only": summary(timed_samples([str(binary), str(bytefile_path(case))], case.rapira_stdin, warmups, samples)),
        }
    machine = machine_metadata()
    return {"timestamp": datetime.now(UTC).isoformat(),
            "revision": git_value(["git", "rev-parse", "HEAD"], "unknown"),
            "dirty": bool(git_value(["git", "status", "--porcelain"], "")),
            "machine": machine, "machine_id": machine_id(machine),
            "rapira_binary": str(binary), "warmups": warmups, "samples": samples,
            "cases": results}


def save_result(result: dict[str, Any]) -> Path:
    """Persist one local run without adding it to the repository history."""
    directory = RESULTS_DIR / result["machine_id"]
    directory.mkdir(parents=True, exist_ok=True)
    timestamp = result["timestamp"].replace(":", "-").replace("+00:00", "Z")
    destination = directory / f"{timestamp}.json"
    destination.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    return destination


def baseline_path(machine: str, name: str) -> Path:
    return RESULTS_DIR / machine / "baselines" / f"{name}.json"


def print_results(result: dict[str, Any], baseline: dict[str, Any] | None = None) -> None:
    print(f"machine: {result['machine_id']}  revision: {result['revision'][:12]}")
    print(
        f"{'case':<16} {'python':>10} {'end-to-end':>12} {'vm-only':>10} "
        f"{'e2e/py':>8} {'vm/py':>8}"
    )
    for name, timings in result["cases"].items():
        values = [timings[key]["median_ns"] / 1_000_000_000
                  for key in ("python", "end_to_end", "vm_only")]
        ratios = (values[1] / values[0], values[2] / values[0])
        suffix = ""
        if baseline and name in baseline["cases"]:
            old = baseline["cases"][name]
            deltas = [(timings[key]["median_ns"] / old[key]["median_ns"] - 1) * 100
                      for key in ("python", "end_to_end", "vm_only")]
            suffix = "  " + ", ".join(f"{delta:+.1f}%" for delta in deltas)
        print(
            f"{name:<16} {values[0]:10.4f} {values[1]:12.4f} {values[2]:10.4f} "
            f"{ratios[0]:8.2f} {ratios[1]:8.2f}{suffix}"
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run, compare, and locally record Rapira benchmarks.")
    parser.add_argument("--case", action="append", default=[], help="benchmark case name (repeatable)")
    parser.add_argument("--tag", action="append", default=[], help="benchmark tag (repeatable)")
    parser.add_argument("--no-build", action="store_true", help="do not build the release CLI")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("verify", help="validate Rapira and Python output")
    commands.add_parser("compile", help="write precompiled RBC files")
    for name, help_text in (("run", "verify, compile, and time all tracks"),
                            ("baseline", "save a local baseline"),
                            ("compare", "compare against a local baseline")):
        command = commands.add_parser(name, help=help_text)
        command.add_argument("--warmups", type=int, default=DEFAULT_WARMUPS)
        command.add_argument("--samples", type=int, default=DEFAULT_SAMPLES)
    commands.choices["baseline"].add_argument("--name", required=True)
    commands.choices["compare"].add_argument("--baseline", required=True)
    profile = commands.add_parser("profile", help="record perf data for one Rapira track")
    profile.add_argument("--mode", choices=("end-to-end", "vm-only"), default="vm-only")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        cases = select_cases(load_cases(), args.case, args.tag)
        binary = rapira_binary()
        build_release(binary, args.no_build)
        if args.command == "verify":
            verify_cases(binary, cases)
            print(f"verified {len(cases)} case(s)")
            return 0
        if args.command == "compile":
            compile_cases(binary, cases)
            print(f"compiled {len(cases)} case(s) into {BUILD_DIR}")
            return 0
        if args.command == "profile":
            if len(cases) != 1:
                raise ValueError("profile requires exactly one selected case")
            case = cases[0]
            verify_cases(binary, cases)
            compile_cases(binary, cases)
            command = source_command(binary, case) if args.mode == "end-to-end" else [str(binary), str(bytefile_path(case))]
            destination = RESULTS_DIR / machine_id(machine_metadata()) / f"perf-{case.name}-{args.mode}.data"
            destination.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run([os.environ.get("PERF", "perf"), "record", "--call-graph", "dwarf",
                            "-o", str(destination), "--", *command], cwd=ROOT,
                           input=case.rapira_stdin, text=True, check=True)
            print(destination)
            return 0
        if args.warmups < 0 or args.samples < 1:
            raise ValueError("warmups must be non-negative and samples must be positive")
        verify_cases(binary, cases)
        compile_cases(binary, cases)
        verify_cases(binary, cases, verify_bytecode=True)
        result = collect_results(binary, cases, args.warmups, args.samples)
        result_path = save_result(result)
        if args.command == "baseline":
            destination = baseline_path(result["machine_id"], args.name)
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
            print_results(result)
            print(f"baseline: {destination}")
        elif args.command == "compare":
            source = baseline_path(result["machine_id"], args.baseline)
            if not source.exists():
                raise ValueError(f"baseline not found for this machine: {source}")
            print_results(result, json.loads(source.read_text()))
        else:
            print_results(result)
        print(f"result: {result_path}")
        return 0
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"benchmark failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
