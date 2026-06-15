#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FORMAT="text"
TITLE="IR Baseline Snapshots"
declare -a ITEMS=()

usage() {
  cat <<'EOF'
usage: scripts/history/render_minor_line_ir_snapshot.sh [--json|--text] [--title TEXT] label=path.gewy [label=path.gewy ...]

Examples:
  scripts/history/render_minor_line_ir_snapshot.sh \
    --title "v0.14.x IR Baseline" \
    amqp-publish=protocols/amqp/publish/main.gewy \
    udp-debug=dsl/udp_process_debug.gewy

  scripts/history/render_minor_line_ir_snapshot.sh --json \
    redis-xadd=protocols/redis/xadd/main.gewy
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --json)
      FORMAT="json"
      ;;
    --text)
      FORMAT="text"
      ;;
    --title)
      shift
      [ "$#" -gt 0 ] || { usage >&2; exit 2; }
      TITLE="$1"
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      ITEMS+=("$1")
      ;;
  esac
  shift
done

[ "${#ITEMS[@]}" -gt 0 ] || { usage >&2; exit 2; }

echo "## ${TITLE}"
echo

for item in "${ITEMS[@]}"; do
  label="${item%%=*}"
  path="${item#*=}"
  if [ -z "${label}" ] || [ "${label}" = "${path}" ]; then
    echo "invalid item '${item}', expected label=path.gewy" >&2
    exit 2
  fi

  rendered="$(
    cd "$ROOT_DIR"
    cargo run --quiet --bin gewyc_ir_snapshot -- "$path" "--${FORMAT}"
  )"

  echo "### ${label}"
  echo
  echo "Source: \`${path}\`"
  echo
  echo "\`\`\`${FORMAT}"
  printf '%s\n' "$rendered"
  echo "\`\`\`"
  echo
done
