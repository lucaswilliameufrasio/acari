#!/usr/bin/env sh
set -eu

usage() {
  cat <<USAGE
Create a feature/bugfix branch following the project conventions.

Usage:
  scripts/start-task.sh <feature|bugfix> <issue-number> <description>

Examples:
  scripts/start-task.sh feature 42 add-exclude-flag
  scripts/start-task.sh bugfix 7 fix-path-traversal

This will:
  1. Ensure you are on 'main' and it is up to date
  2. Create and checkout a branch:
     feature/issue-42-add-exclude-flag

Then you can make atomic commits and open a PR.
USAGE
}

if [ "$#" -ne 3 ]; then
  usage
  exit 1
fi

TYPE="$1"
ISSUE="$2"
DESCRIPTION="$3"

# Validate type
if [ "$TYPE" != "feature" ] && [ "$TYPE" != "bugfix" ]; then
  echo "Error: type must be 'feature' or 'bugfix', got '$TYPE'" >&2
  exit 1
fi

# Validate issue number
if ! echo "$ISSUE" | grep -qE '^[0-9]+$'; then
  echo "Error: issue number must be numeric, got '$ISSUE'" >&2
  exit 1
fi

# Validate description (kebab-case, alphanumeric + hyphens + underscores)
if ! echo "$DESCRIPTION" | grep -qE '^[-_a-zA-Z0-9]+$'; then
  echo "Error: description must be kebab-case (alphanumeric, hyphens, underscores), got '$DESCRIPTION'" >&2
  exit 1
fi

BRANCH="${TYPE}/issue-${ISSUE}-${DESCRIPTION}"

# Ensure we're on main and up to date
CURRENT_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [ "$CURRENT_BRANCH" != "main" ]; then
  echo "Warning: you are on '$CURRENT_BRANCH', not 'main'."
  echo "The branch will be created from HEAD."
fi

if ! git diff --quiet; then
  echo "Error: you have unstaged changes. Commit or stash them first." >&2
  exit 1
fi

git checkout -b "$BRANCH"

echo ""
echo "Branch created: $BRANCH"
echo ""
echo "Next steps:"
echo "  1. Make atomic commits as you work"
echo "  2. When ready, rebase + squash into one commit:"
echo "     git rebase -i origin/main"
echo "  3. Open a PR:"
echo "     git push --set-upstream origin $BRANCH"
