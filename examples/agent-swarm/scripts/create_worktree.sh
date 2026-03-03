#!/bin/bash
# create_worktree.sh
# Safely creates a new git worktree inside the selected repo's .swarm/worktrees/ directory
# Usage: ./create_worktree.sh <task_id>

set -e

if [ -z "$1" ]; then
    echo "Error: task_id is required."
    echo "Usage: $0 <task_id>"
    exit 1
fi

TASK_ID=$1
BASE_TASK_ID=$TASK_ID

# Validate task ID format to prevent path traversal
if ! [[ "$TASK_ID" =~ ^[a-zA-Z0-9_-]+$ ]]; then
    echo "Error: Invalid task_id format. Only alphanumeric, dashes, and underscores are allowed."
    exit 1
fi

# Resolve repository root from current working directory
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

# Ensure parent directory exists inside selected repo
mkdir -p .swarm/worktrees

# Create a unique worktree/branch if prior runs already used this task id.
UNIQ="$BASE_TASK_ID"
SUFFIX=1
while true; do
    WORKTREE_DIR=".swarm/worktrees/$UNIQ"
    BRANCH_NAME="swarm/$UNIQ"
    HAS_BRANCH=0
    HAS_WORKTREE=0
    if git show-ref --verify --quiet "refs/heads/$BRANCH_NAME"; then
        HAS_BRANCH=1
    fi
    if [ -d "$WORKTREE_DIR" ]; then
        HAS_WORKTREE=1
    fi
    if [ "$HAS_BRANCH" -eq 0 ] && [ "$HAS_WORKTREE" -eq 0 ]; then
        break
    fi
    UNIQ="${BASE_TASK_ID}-${SUFFIX}"
    SUFFIX=$((SUFFIX + 1))
done

echo "Creating new git worktree for $BRANCH_NAME at $WORKTREE_DIR"

# Create the worktree from repo root
git worktree add -b "$BRANCH_NAME" "$WORKTREE_DIR"

# Return the absolute path so the Manager Agent knows where to send the Worker
ABSOLUTE_PATH=$(cd "$WORKTREE_DIR" && pwd)

echo "Success!"
echo "worktreePath=$ABSOLUTE_PATH"
echo "branch=$BRANCH_NAME"
echo "WORKTREE_PATH=$ABSOLUTE_PATH"
echo "BRANCH=$BRANCH_NAME"
