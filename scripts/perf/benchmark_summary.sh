#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
ROUNDS="${1:-5}"
FILTER="${2:-benchmark_}"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-bench.XXXXXX")"
trap 'rm -rf "$TMP_DIR"' EXIT

if ! [[ "$ROUNDS" =~ ^[0-9]+$ ]] || [ "$ROUNDS" -lt 1 ]; then
  echo "rounds must be a positive integer" >&2
  exit 2
fi

echo "gewyvern benchmark summary"
echo "  rounds: $ROUNDS"
echo "  filter: $FILTER"
echo

run_bench_round() {
  local round="$1"
  local output_file="$TMP_DIR/round-$round.log"

  echo "== round $round/$ROUNDS =="
  (
    cd "$ROOT_DIR"
    cargo test --workspace "$FILTER" -- --ignored --nocapture --test-threads=1
  ) 2>&1 | tee "$output_file"

  grep -o 'benchmark_[^:[:space:]]*:[^[:cntrl:]]*elapsed_ms=[0-9.]*' "$output_file" | while IFS= read -r line; do
    name="${line%%:*}"
    value="${line##*elapsed_ms=}"
    printf '%s\n' "$value" >> "$TMP_DIR/$name.values"
  done
  echo
}

for round in $(seq 1 "$ROUNDS"); do
  run_bench_round "$round"
done

echo "== aggregated summary =="
found_any=0
while IFS= read -r values_file; do
  found_any=1
  bench_name="$(basename "$values_file" .values)"
  summary="$(
    sort -n "$values_file" | awk '
      {
        values[NR] = $1
        sum += $1
      }
      END {
        if (NR == 0) {
          exit 1
        }
        if (NR % 2 == 1) {
          median = values[(NR + 1) / 2]
        } else {
          median = (values[NR / 2] + values[(NR / 2) + 1]) / 2
        }
        printf "n=%d min=%.3f median=%.3f max=%.3f avg=%.3f", NR, values[1], median, values[NR], sum / NR
      }
    '
  )"
  echo "$bench_name: $summary"
done < <(find "$TMP_DIR" -name 'benchmark_*.values' | sort)

if [ "$found_any" -eq 0 ]; then
  echo "no benchmark output matched filter '$FILTER'" >&2
  exit 1
fi
