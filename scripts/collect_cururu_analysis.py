"""Collect a bounded advisory status from GitHub, never from PR artifacts."""

import json
import os
from pathlib import Path
import re
import subprocess
from urllib.parse import quote

from cururu_analysis_status import classify_run


def gh_api(path):
    result = subprocess.run(
        ["gh", "api", "-H", "Accept: application/vnd.github+json", path],
        check=True, capture_output=True, text=True, timeout=30,
    )
    if len(result.stdout) > 4_000_000:
        raise ValueError("API response too large")
    return json.loads(result.stdout)


def collect(api, repo, pr_number, expected_sha):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repo):
        raise ValueError("invalid repository")
    if not re.fullmatch(r"[1-9][0-9]*", pr_number) or not re.fullmatch(r"[0-9a-f]{40}", expected_sha):
        raise ValueError("invalid PR identity")
    base = f"repos/{repo}"
    pr_path = f"{base}/pulls/{pr_number}"
    pr = api(pr_path)
    if pr["head"]["sha"] != expected_sha:
        raise ValueError("PR head changed")
    repo_id = pr["base"]["repo"]["id"]
    head_repo_id = pr["head"]["repo"]["id"]
    workflow = api(f"{base}/actions/workflows/cururu-analysis.yml")
    if workflow.get("path") != ".github/workflows/cururu-analysis.yml":
        raise ValueError("unexpected workflow path")
    listing = api(f"{base}/actions/workflows/cururu-analysis.yml/runs?event=pull_request&head_sha={quote(expected_sha)}&per_page=100")
    runs = listing["workflow_runs"]
    state = "unknown"
    if isinstance(runs, list) and type(listing.get("total_count")) is int and listing["total_count"] == len(runs):
        candidates = []
        ambiguous = False
        seen_ids = set()
        for run in runs:
            if not isinstance(run, dict) or not isinstance(run.get("head_sha"), str):
                ambiguous = True
                continue
            if run["head_sha"] != expected_sha:
                continue
            prs = run.get("pull_requests")
            if not isinstance(prs, list) or not prs or any(not isinstance(p, dict) or type(p.get("number")) is not int for p in prs):
                ambiguous = True
                continue
            if len(prs) != 1:
                ambiguous = True
                continue
            if not any(p["number"] == int(pr_number) for p in prs):
                continue
            if type(run.get("id")) is not int or run["id"] <= 0:
                raise ValueError("invalid run id")
            if run["id"] in seen_ids:
                ambiguous = True
            seen_ids.add(run["id"])
            candidates.append(run)
        if candidates and not ambiguous:
            newest = max(candidates, key=lambda run: run["id"])
            state = classify_run(newest, expected_sha=expected_sha, workflow_id=workflow["id"],
                                 repository_id=repo_id, head_repository_id=head_repo_id)
    current = api(pr_path)
    if (current["head"]["sha"] != expected_sha or current["base"]["repo"]["id"] != repo_id
            or current["head"]["repo"]["id"] != head_repo_id):
        raise ValueError("PR identity changed during lookup")
    # A PR can modify its workflow; even a matching successful run is not trusted proof.
    tool_status = "failed" if state == "failed" else "not_run"
    message = "GitHub reported analyzer failure" if state == "failed" else "Analyzer evidence unavailable or untrusted"
    return {"schema_version": 1, "commit_sha": expected_sha,
            "tools": [{"name": "cururu-pr-analysis", "status": tool_status, "message": message}]}


def main():
    result = collect(gh_api, os.environ["GITHUB_REPOSITORY"], os.environ["PR_NUMBER"], os.environ["EXPECTED_HEAD_SHA"])
    directory = Path(os.environ["GITHUB_WORKSPACE"]) / ".cururu-evidence"
    directory.mkdir(mode=0o700)  # fail if anything already occupies the path
    with (directory / "analysis.json").open("x", encoding="utf-8") as target:
        json.dump(result, target)


if __name__ == "__main__":
    main()
