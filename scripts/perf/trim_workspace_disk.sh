#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"

DRY_RUN=0

if [ "${1:-}" = "--dry-run" ]; then
  DRY_RUN=1
elif [ "${1:-}" != "" ]; then
  echo "usage: bash scripts/perf/trim_workspace_disk.sh [--dry-run]" >&2
  exit 2
fi

find_cleanup_dirs() {
  emit_if_untracked() {
    local path="$1"
    local relative_path="${path#"$ROOT_DIR"/}"

    if [ -z "$(git -C "$ROOT_DIR" ls-files -- "$relative_path")" ]; then
      printf '%s\n' "$path"
    fi
  }

  {
    while IFS= read -r -d '' cache_dir; do
      emit_if_untracked "$cache_dir"
    done < <(
      find "$ROOT_DIR" \
        \( \
          -path "$ROOT_DIR/.git" -o \
          -path "$ROOT_DIR/target" -o \
          -path "$ROOT_DIR/apps/leserpent/src/Leserpent/data" \
        \) -prune -o \
        -type d \
        \( -name node_modules -o -name __pycache__ -o -name .pytest_cache \) \
        -print0 -prune
    )

    while IFS= read -r -d '' project; do
      project_dir="$(dirname "$project")"
      for output_dir in "$project_dir/bin" "$project_dir/obj"; do
        if [ -d "$output_dir" ]; then
          emit_if_untracked "$output_dir"
        fi
      done
    done < <(
      find "$ROOT_DIR" \
        \( -path "$ROOT_DIR/.git" -o -path "$ROOT_DIR/target" \) -prune -o \
        -type f -name '*.csproj' -print0
    )
  } | sort -u
}

cleanup_dirs="$(find_cleanup_dirs)"

echo "gewyvern workspace disk trim"
echo "  root: $ROOT_DIR"
echo

if [ -d "$ROOT_DIR/target" ]; then
  echo "cargo target:"
  du -sh "$ROOT_DIR/target"
else
  echo "cargo target:"
  echo "  already clean"
fi

if [ -n "$cleanup_dirs" ]; then
  echo
  echo "rebuildable directories:"
  while IFS= read -r path; do
    du -sh "$path"
  done <<< "$cleanup_dirs"
else
  echo
  echo "rebuildable directories:"
  echo "  nothing to remove"
fi

if [ "$DRY_RUN" -eq 1 ]; then
  echo
  echo "dry run only, no files removed"
  exit 0
fi

echo
echo "removing Rust build output..."
if [ -f "$ROOT_DIR/Cargo.toml" ]; then
  (
    cd "$ROOT_DIR"
    cargo clean
  )
fi

if [ -n "$cleanup_dirs" ]; then
  echo
  echo "removing rebuildable directories..."
  while IFS= read -r path; do
    rm -rf "$path"
    echo "  removed $path"
  done <<< "$cleanup_dirs"
fi

echo
echo "top-level usage after cleanup:"
du -sh "$ROOT_DIR"/* "$ROOT_DIR"/.git 2>/dev/null | sort -hr | sed -n '1,20p'
