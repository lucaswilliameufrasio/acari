#!/bin/sh
set -eu

case "${EVENT_NAME:-}" in
  pull_request_target)
    head_sha=${EVENT_HEAD_SHA:-}
    ;;
  issue_comment)
    case "${PR_NUMBER:-}" in
      ''|*[!0-9]*) echo 'Invalid pull request number' >&2; exit 1 ;;
    esac
    head_sha=$(gh api "repos/${GITHUB_REPOSITORY}/pulls/${PR_NUMBER}" --jq '.head.sha')
    ;;
  *) echo 'Unsupported event' >&2; exit 1 ;;
esac

case "$head_sha" in
  *[!0-9a-f]*|'') echo 'Invalid pull request head SHA' >&2; exit 1 ;;
esac
if [ "${#head_sha}" -ne 40 ]; then
  echo 'Invalid pull request head SHA length' >&2
  exit 1
fi
printf 'sha=%s\n' "$head_sha" >> "${GITHUB_OUTPUT:?}"
