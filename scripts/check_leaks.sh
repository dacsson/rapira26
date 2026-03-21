#!/bin/bash
# Usage: scripts/check_leaks.sh <source.rap>
# Compiles and runs a Rapira program with leak tracking enabled.

set -e

if [ $# -eq 0 ]; then
    echo "Usage: $0 <source.rap>"
    exit 1
fi

SOURCE="$1"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
RUNTIME_DIR="$PROJECT_DIR/runtime"
TMP_DIR=$(mktemp -d)

cleanup() {
    rm -rf "$TMP_DIR"
}
trap cleanup EXIT

# 1. Build runtime with leak tracking
make -C "$RUNTIME_DIR" clean > /dev/null 2>&1
make -C "$RUNTIME_DIR" CFLAGS="-DRAP_TEST_LEAKS" > /dev/null 2>&1

# 2. Generate C with --check-leaks
cargo run -q --manifest-path "$PROJECT_DIR/Cargo.toml" -- --check-leaks --emit-c "$SOURCE" > "$TMP_DIR/program.c"

# 3. Compile
gcc "$TMP_DIR/program.c" -o "$TMP_DIR/program" \
    -I"$RUNTIME_DIR" -L"$RUNTIME_DIR/lib" -lrapruntime -lm

# 4. Run and capture stderr
set +e
"$TMP_DIR/program" 2> "$TMP_DIR/stderr.txt"
EXIT_CODE=$?
set -e

STDERR=$(cat "$TMP_DIR/stderr.txt")

# 5. Rebuild runtime without leak tracking
make -C "$RUNTIME_DIR" clean > /dev/null 2>&1
make -C "$RUNTIME_DIR" > /dev/null 2>&1

# 6. Report
if echo "$STDERR" | grep -q "LEAK"; then
    echo "LEAK DETECTED in $SOURCE"
    echo "$STDERR"
    exit 1
else
    echo "No leaks in $SOURCE"
    exit 0
fi
