#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:-}"
task_id="${2:-}"
base_ref="${3:-HEAD}"

if [[ -z "$repo_root" || -z "$task_id" ]]; then
  echo "usage: $0 <repo_root> <task_id> [base_ref]" >&2
  exit 2
fi

repo_root="$(cd "$repo_root" && pwd)"
worktrees_root="$repo_root/.swarm/worktrees"
mkdir -p "$worktrees_root"

safe_task_id="$(echo "$task_id" | tr '[:upper:]' '[:lower:]' | tr -c 'a-z0-9._-' '-' | sed -E 's/^-+//; s/-+$//')"
if [[ -z "$safe_task_id" ]]; then
  safe_task_id="task"
fi
branch="swarm/${safe_task_id}"
worktree_path="$worktrees_root/$safe_task_id"

canon_repo="$repo_root"
canon_target_parent="$(cd "$worktrees_root" && pwd)"
canon_target="$canon_target_parent/$safe_task_id"
case "$canon_target" in
  "$canon_repo"/*) ;;
  *)
    echo "refusing path outside repository root: $canon_target" >&2
    exit 3
    ;;
esac

if [[ -d "$worktree_path/.git" || -f "$worktree_path/.git" ]]; then
  echo "ok=true"
  echo "created=false"
  echo "worktreePath=$worktree_path"
  echo "branch=$branch"
  exit 0
fi

if git -C "$repo_root" show-ref --verify --quiet "refs/heads/$branch"; then
  git -C "$repo_root" worktree add "$worktree_path" "$branch" >/dev/null
else
  git -C "$repo_root" worktree add -b "$branch" "$worktree_path" "$base_ref" >/dev/null
fi

echo "ok=true"
echo "created=true"
echo "worktreePath=$worktree_path"
echo "branch=$branch"
