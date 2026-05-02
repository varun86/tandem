#!/usr/bin/env bash

set -euo pipefail

REPO="${REPO:-}"
STATE="${STATE:-open}"
LABEL="${LABEL:-}"
AUTHOR="${AUTHOR:-}"
EXTRA_QUERY="${EXTRA_QUERY:-}"
BODY_CONTAINS="${BODY_CONTAINS:-}"
MAX_ISSUES="${MAX_ISSUES:-0}"
DRY_RUN=1
CONFIRM=1
BATCH_SIZE=100
VERBOSE=0

usage() {
  cat <<'EOF'
Usage: ./scripts/delete_github_issues.sh [--repo OWNER/REPO] [options]

Options:
  --repo OWNER/REPO          Target repository (default: current gh repo)
  --state open|closed|all    Issue state filter (default: open)
  --label LABEL              Restrict to a label
  --author LOGIN             Restrict to issues opened by a user
  --query "TEXT"             Extra GitHub issue search text
  --body-contains TEXT       Only delete issues whose body contains TEXT
  --max N                    Maximum number of issues to process (0 = unlimited)
  --verbose                  Print each API request/page and totals
  --apply                    Actually delete after an interactive confirmation
  --yes                      Actually delete without an interactive confirmation
  --help                     Show this message

Environment:
  GH_TOKEN / GITHUB_TOKEN     Required by gh (if not already authenticated)
  Optional default query pieces can also be set via:
    REPO, STATE, LABEL, AUTHOR, BODY_CONTAINS, MAX_ISSUES

Notes:
  - Uses GitHub GraphQL deleteIssue mutation; this permanently deletes issues.
  - The script is intentionally conservative and defaults to dry-run mode.
EOF
}

# Example dry run:
# ./scripts/delete_github_issues.sh --repo OWNER/REPO --state open --query "created:2026-05-01..2026-05-31" --max 300

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      REPO="$2"
      shift 2
      ;;
    --state)
      STATE="$2"
      shift 2
      ;;
    --label)
      LABEL="$2"
      shift 2
      ;;
    --author)
      AUTHOR="$2"
      shift 2
      ;;
    --query)
      EXTRA_QUERY="$2"
      shift 2
      ;;
    --body-contains)
      BODY_CONTAINS="$2"
      shift 2
      ;;
    --max)
      MAX_ISSUES="$2"
      shift 2
      ;;
    --verbose)
      VERBOSE=1
      shift
      ;;
    --apply|--yes)
      DRY_RUN=0
      if [[ "$1" == "--yes" ]]; then
        CONFIRM=0
      fi
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      usage
      exit 1
      ;;
  esac
done

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh CLI is required. Install from https://cli.github.com/."
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "Error: jq is required."
  exit 1
fi

if [[ -z "$REPO" ]]; then
  REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner)"
  if [[ -z "$REPO" ]]; then
    echo "Error: --repo is required when not run inside a git repo with gh remotes."
    exit 1
  fi
fi

if [[ "$STATE" != "open" && "$STATE" != "closed" && "$STATE" != "all" ]]; then
  echo "Error: --state must be open, closed, or all."
  exit 1
fi
if ! [[ "$MAX_ISSUES" =~ ^[0-9]+$ ]]; then
  echo "Error: --max must be a non-negative integer."
  exit 1
fi
if ! [[ "$BATCH_SIZE" =~ ^[0-9]+$ ]] || [[ "$BATCH_SIZE" -lt 1 || "$BATCH_SIZE" -gt 100 ]]; then
  echo "Error: batch size must be between 1 and 100."
  exit 1
fi

SEARCH_QUERY="repo:${REPO} is:issue"
if [[ "$STATE" != "all" ]]; then
  SEARCH_QUERY+=" state:${STATE}"
fi
if [[ -n "$LABEL" ]]; then
  SEARCH_QUERY+=" label:${LABEL}"
fi
if [[ -n "$AUTHOR" ]]; then
  SEARCH_QUERY+=" author:${AUTHOR}"
fi
if [[ -n "$EXTRA_QUERY" ]]; then
  SEARCH_QUERY+=" ${EXTRA_QUERY}"
fi

echo "Repository: ${REPO}"
echo "Search query: ${SEARCH_QUERY}"
if [[ "${MAX_ISSUES}" -gt 0 ]]; then
  echo "Max issues: ${MAX_ISSUES}"
else
  echo "Max issues: unlimited"
fi
if [[ -n "$BODY_CONTAINS" ]]; then
  echo "Body contains: ${BODY_CONTAINS}"
fi
if [[ $DRY_RUN -eq 1 ]]; then
  echo "Mode: dry-run"
else
  echo "Mode: destructive delete"
fi
echo

if [[ $DRY_RUN -eq 0 && $CONFIRM -eq 1 ]]; then
  echo "About to delete issues from: $REPO"
  echo "Query: $SEARCH_QUERY"
  read -r -p "This operation is destructive and irreversible. Type 'delete' to continue: " confirmation
  if [[ "$confirmation" != "delete" ]]; then
    echo "Aborted."
    exit 0
  fi
fi

if [[ $VERBOSE -eq 1 ]]; then
  echo "Repository: ${REPO}"
  echo "State: ${STATE}"
  if [[ -n "${LABEL}" ]]; then
    echo "Label: ${LABEL}"
  fi
  if [[ -n "${AUTHOR}" ]]; then
    echo "Author: ${AUTHOR}"
  fi
  if [[ -n "${EXTRA_QUERY}" ]]; then
    echo "Extra query: ${EXTRA_QUERY}"
  fi
  if [[ -n "${BODY_CONTAINS}" ]]; then
    echo "Body contains: ${BODY_CONTAINS}"
  fi
  echo "Search query: ${SEARCH_QUERY}"
  echo "Batch size: ${BATCH_SIZE}"
  if [[ "${MAX_ISSUES}" -gt 0 ]]; then
    echo "Max: ${MAX_ISSUES}"
  else
    echo "Max: unlimited"
  fi
  if [[ $DRY_RUN -eq 1 ]]; then
    echo "Mode: dry-run"
  else
    echo "Mode: destructive (delete)"
  fi
  echo
fi

page=1
scanned=0
skipped=0
processed=0
deleted=0
total_reported=0

delete_issue() {
  local id="$1"
  gh api graphql \
    -f query='mutation($issueId: ID!) { deleteIssue(input: { issueId: $issueId }) { clientMutationId } }' \
    -f issueId="$id" >/dev/null
}

while true; do
  if [[ $VERBOSE -eq 1 ]]; then
    echo "Fetching page ${page}..."
  fi
  page_json="$(mktemp)"
  gh api --method GET \
    /search/issues \
    --field q="$SEARCH_QUERY" \
    --field per_page="$BATCH_SIZE" \
    --field page="$page" \
    > "$page_json"

  item_count="$(jq -r '.items | length' "$page_json")"
  total_count="$(jq -r '.total_count // 0' "$page_json")"
  if [[ $total_reported -eq 0 && $VERBOSE -eq 1 ]]; then
    total_reported=1
    echo "Search matched ${total_count} issues total."
  fi
  if [[ "$item_count" -eq 0 ]]; then
    if [[ $page -eq 1 ]]; then
      echo "No issues matched that query."
    fi
    rm -f "$page_json"
    break
  fi

  while IFS= read -r encoded_issue; do
    issue="$(printf '%s' "$encoded_issue" | base64 --decode)"
    number="$(jq -r '.number' <<< "$issue")"
    node_id="$(jq -r '.node_id' <<< "$issue")"
    title="$(jq -r '.title' <<< "$issue")"
    url="$(jq -r '.html_url' <<< "$issue")"
    body="$(jq -r '.body // ""' <<< "$issue")"

    (( ++scanned ))
    if [[ -n "$BODY_CONTAINS" && "$body" != *"$BODY_CONTAINS"* ]]; then
      (( ++skipped ))
      if [[ $VERBOSE -eq 1 ]]; then
        echo "Skipping #${number}: body marker not found"
      fi
      continue
    fi

    (( ++processed ))
    echo "#${number} ${title} ${url}"

    if [[ $DRY_RUN -eq 1 ]]; then
      echo "  - would delete"
    else
      if delete_issue "$node_id"; then
        (( ++deleted ))
        echo "  - deleted"
      else
        echo "  - failed to delete #${number}" >&2
      fi
    fi

    if [[ "$MAX_ISSUES" -gt 0 && "$processed" -ge "$MAX_ISSUES" ]]; then
      rm -f "$page_json"
      if [[ $DRY_RUN -eq 1 ]]; then
        echo "Dry run limit reached: ${processed} issues would be processed; scanned ${scanned}, skipped ${skipped}."
      else
        echo "Deleted ${deleted} issues (limited by --max); scanned ${scanned}, skipped ${skipped}."
      fi
      exit 0
    fi
  done < <(jq -r '.items[] | @base64' "$page_json")

  rm -f "$page_json"
  (( page++ ))
done

if [[ $DRY_RUN -eq 1 ]]; then
  echo "Dry run complete: ${processed} issue(s) would be deleted; scanned ${scanned}, skipped ${skipped}."
else
  echo "Done. Deleted ${deleted}/${processed} issue(s); scanned ${scanned}, skipped ${skipped}."
fi
