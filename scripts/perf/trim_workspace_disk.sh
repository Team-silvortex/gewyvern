#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd -P)"

DRY_RUN=1

if [ "$#" -gt 1 ]; then
  echo "usage: bash scripts/perf/trim_workspace_disk.sh [--dry-run|--apply]" >&2
  exit 2
fi

case "${1:-}" in
  ""|--dry-run) ;;
  --apply) DRY_RUN=0 ;;
  *)
    echo "usage: bash scripts/perf/trim_workspace_disk.sh [--dry-run|--apply]" >&2
    exit 2
    ;;
esac

for output_dir in "$ROOT_DIR/target" "$ROOT_DIR/target/debug" "$ROOT_DIR/target/lean"; do
  if [ -L "$output_dir" ]; then
    echo "refusing symlinked Cargo output: $output_dir" >&2
    exit 2
  fi
done

find_cleanup_dirs() {
  emit_if_rebuildable() {
    local path="$1"
    local relative_path="${path#"$ROOT_DIR"/}"

    if [ -d "$path" ] && [ ! -L "$path" ] && \
      git -C "$ROOT_DIR" check-ignore -q -- "$relative_path" && \
      [ -z "$(git -C "$ROOT_DIR" ls-files -- "$relative_path")" ]; then
      printf '%s\n' "$path"
    fi
  }

  {
    while IFS= read -r -d '' cache_dir; do
      emit_if_rebuildable "$cache_dir"
    done < <(
      find "$ROOT_DIR" \
        \( \
          -path "$ROOT_DIR/.git" -o \
          -path "$ROOT_DIR/target" -o \
          -path "$ROOT_DIR/artifacts" -o \
          -path "$ROOT_DIR/apps/leserpent-avalonia/osx-arm64" -o \
          -name '*.app' -o \
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
          emit_if_rebuildable "$output_dir"
        fi
      done
    done < <(
      find "$ROOT_DIR" \
        \( \
          -path "$ROOT_DIR/.git" -o \
          -path "$ROOT_DIR/target" -o \
          -path "$ROOT_DIR/artifacts" -o \
          -path "$ROOT_DIR/apps/leserpent-avalonia/osx-arm64" -o \
          -name '*.app' -o \
          -path "$ROOT_DIR/apps/leserpent/src/Leserpent/data" \
        \) -prune -o \
        -type f -name '*.csproj' -print0
    )
  } | sort -u
}

cleanup_dirs="$(find_cleanup_dirs)"

echo "gewyvern workspace disk trim"
echo "  root: $ROOT_DIR"
echo

echo "rebuildable Cargo profiles (release, packages and validation evidence are retained):"
for profile_dir in "$ROOT_DIR/target/debug" "$ROOT_DIR/target/lean"; do
  if [ -d "$profile_dir" ]; then
    du -sh "$profile_dir"
  fi
done

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
  echo "dry run only, no files removed; use --apply while builds and applications are idle"
  exit 0
fi

echo
echo "removing Rust build output..."
for profile in dev lean; do
  profile_dir="$ROOT_DIR/target/$profile"
  if [ "$profile" = dev ]; then
    profile_dir="$ROOT_DIR/target/debug"
  fi
  if [ -d "$profile_dir" ]; then
    (
      cd "$ROOT_DIR"
      cargo clean --locked --offline --profile "$profile" --target-dir "$ROOT_DIR/target"
    )
  fi
done

if [ -n "$cleanup_dirs" ]; then
  echo
  echo "removing rebuildable directories..."
  while IFS= read -r path; do
    if [ -L "$path" ] || \
      ! git -C "$ROOT_DIR" check-ignore -q -- "${path#"$ROOT_DIR"/}" || \
      [ -n "$(git -C "$ROOT_DIR" ls-files -- "${path#"$ROOT_DIR"/}")" ]; then
      echo "refusing changed cleanup candidate: $path" >&2
      exit 2
    fi
    rm -rf -- "$path"
    echo "  removed $path"
  done <<< "$cleanup_dirs"
fi

echo
echo "top-level usage after cleanup:"
du -sh "$ROOT_DIR"/* "$ROOT_DIR"/.git 2>/dev/null | sort -hr | sed -n '1,20p'
