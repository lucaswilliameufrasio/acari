#!/usr/bin/env sh
set -eu

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { printf "${CYAN}%s${NC}\n" "$*"; }
ok()    { printf "${GREEN}✓${NC} %s\n" "$*"; }
warn()  { printf "${YELLOW}⚠ %s${NC}\n" "$*"; }
err()   { printf "${RED}✗ %s${NC}\n" "$*"; exit 1; }

prompt() {
  local msg="$1" default="$2"
  if [ -n "$default" ]; then
    printf "%s [%s]: " "$msg" "$default" >&2
  else
    printf "%s: " "$msg" >&2
  fi
  read -r input
  if [ -z "$input" ]; then
    printf "%s" "$default"
  else
    printf "%s" "$input"
  fi
}

kebab() {
  printf "%s" "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed 's/[^a-zA-Z0-9]/-/g' \
    | sed 's/--*/-/g' \
    | sed 's/^-//; s/-$//'
}

main() {
  echo ""
  info "╔══════════════════════════════════════╗"
  info "║     Acari — Start a new task         ║"
  info "╚══════════════════════════════════════╝"
  echo ""

  # --- Collect description ---
  info "What do you want to do? (describe the task)"
  info "Example: add exclude flag to scanner"
  printf "> " >&2
  read -r raw_description

  if [ -z "$raw_description" ]; then
    err "Description cannot be empty."
  fi

  # --- Suggest type ---
  local suggested_type="feature"
  case "$raw_description" in
    *fix*|*bug*|*error*|*crash*|*regression*|*broken*) suggested_type="bugfix" ;;
    *add*|*implement*|*create*|*new*|*support*)        suggested_type="feature" ;;
  esac

  local type
  type=$(prompt "Task type" "$suggested_type")
  case "$type" in
    feat*) type="feature" ;;
    bug*)  type="bugfix" ;;
  esac
  if [ "$type" != "feature" ] && [ "$type" != "bugfix" ]; then
    err "Type must be 'feature' or 'bugfix', got '$type'"
  fi

  # --- Suggest issue number (optional) ---
  local suggested_issue=""
  local issue_match
  issue_match=$(printf "%s" "$raw_description" | grep -oE '[0-9]{2,}' | head -1 || true)
  if [ -n "$issue_match" ]; then
    suggested_issue="$issue_match"
  fi

  local issue
  issue=$(prompt "Issue number (optional)" "${suggested_issue:-}")

  # --- Generate branch name ---
  local description_kebab
  description_kebab=$(kebab "$raw_description")

  local branch
  if [ -n "$issue" ]; then
    branch="${type}/issue-${issue}-${description_kebab}"
  else
    branch="${type}/${description_kebab}"
  fi

  echo ""
  info "Suggested branch name:"
  printf "  ${GREEN}%s${NC}\n" "$branch"
  echo ""

  # --- Confirm and create ---
  printf "Create branch? [Y/n] "
  read -r confirm
  if [ "$confirm" = "n" ] || [ "$confirm" = "N" ]; then
    info "Cancelled."
    exit 0
  fi

  if ! git diff --quiet; then
    err "You have unstaged changes. Commit or stash them first."
  fi

  git checkout -b "$branch"

  echo ""
  ok "Branch created: $branch"
  echo ""
  echo "Next steps:"
  echo "  1. Make atomic commits as you work"
  echo "  2. When ready, rebase + squash into one commit:"
  echo "     git rebase -i origin/main"
  echo "  3. Open a PR:"
  echo "     git push --set-upstream origin $branch"
}

main
