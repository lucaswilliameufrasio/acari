"""Conservative projection of GitHub workflow-run metadata; not an API client.

The caller must obtain the metadata and expected identities from GitHub's API,
not from a pull-request artifact. A result is advisory, not proof of integrity.
"""

import re


def classify_run(run, *, expected_sha, workflow_id, repository_id, head_repository_id):
    """Return passed, failed, or unknown for one exact workflow attempt."""
    if not isinstance(run, dict) or not isinstance(expected_sha, str) or not re.fullmatch(r"[0-9a-f]{40}", expected_sha):
        return "unknown"
    if any(type(value) is not int or value <= 0 for value in (workflow_id, repository_id, head_repository_id)):
        return "unknown"
    repository = run.get("repository")
    head_repository = run.get("head_repository")
    if not isinstance(repository, dict) or not isinstance(head_repository, dict):
        return "unknown"
    if (
        run.get("event") != "pull_request"
        or type(run.get("workflow_id")) is not int
        or run["workflow_id"] != workflow_id
        or type(repository.get("id")) is not int
        or repository["id"] != repository_id
        or type(head_repository.get("id")) is not int
        or head_repository["id"] != head_repository_id
        or run.get("head_sha") != expected_sha
        or type(run.get("id")) is not int
        or run["id"] <= 0
        or type(run.get("run_attempt")) is not int
        or run["run_attempt"] <= 0
        or run.get("status") != "completed"
    ):
        return "unknown"
    if run.get("conclusion") == "success":
        return "passed"
    if run.get("conclusion") in ("failure", "timed_out", "action_required", "startup_failure"):
        return "failed"
    return "unknown"
