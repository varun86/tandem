#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:-}"
if [[ -z "$repo_root" ]]; then
  echo "usage: $0 <repo_root>" >&2
  exit 2
fi

repo_root="$(cd "$repo_root" && pwd)"
worktrees_root="$repo_root/.swarm/worktrees"

if [[ ! -d "$worktrees_root" ]]; then
  echo "no managed worktrees found"
  exit 0
fi

find "$worktrees_root" -mindepth 1 -maxdepth 1 -type d | while read -r wt; do
  canon="$(cd "$wt" && pwd)"
  case "$canon" in
    "$repo_root"/*) ;;
    *)
      echo "skip outside root: $canon" >&2
      continue
      ;;
  esac
  git -C "$repo_root" worktree remove --force "$canon" || true
done

echo "cleanup complete"
