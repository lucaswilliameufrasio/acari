import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from collect_cururu_analysis import collect, gh_api, main


SHA = "a" * 40
PR = {"head": {"sha": SHA, "repo": {"id": 13}}, "base": {"repo": {"id": 12}}}
RUN = {"id": 25, "run_attempt": 1, "event": "pull_request", "workflow_id": 11,
       "repository": {"id": 12}, "head_repository": {"id": 13}, "head_sha": SHA,
       "pull_requests": [{"number": 42}], "status": "completed", "conclusion": "failure"}


def api_fixture(runs, second_pr=None, total=None, first_pr=None):
    count = 0

    def api(path):
        nonlocal count
        if "/pulls/" in path:
            count += 1
            original = PR if first_pr is None else first_pr
            return copy.deepcopy(second_pr if count == 2 and second_pr is not None else original)
        if "?event=" in path:
            return {"workflow_runs": copy.deepcopy(runs), "total_count": len(runs) if total is None else total}
        return {"id": 11, "path": ".github/workflows/cururu-analysis.yml"}
    return api


class CollectTests(unittest.TestCase):
    def result(self, runs, **kwargs):
        return collect(api_fixture(runs, **kwargs), "owner/repo", "42", SHA)

    def test_verified_failure_is_recorded_without_untrusted_text(self):
        run = {**RUN, "message": "untrusted payload"}
        manifest = self.result([run])
        self.assertEqual(manifest["commit_sha"], SHA)
        self.assertEqual(manifest["tools"][0]["status"], "failed")
        self.assertNotIn("untrusted payload", str(manifest))

    def test_success_and_pending_never_become_passed(self):
        for state, conclusion in [("completed", "success"), ("in_progress", None), ("completed", "cancelled")]:
            manifest = self.result([{**RUN, "status": state, "conclusion": conclusion}])
            self.assertEqual(manifest["tools"][0]["status"], "not_run")

    def test_mismatched_or_incomplete_evidence_is_unknown(self):
        for run in [{**RUN, "head_sha": "b" * 40}, {**RUN, "workflow_id": 99},
                    {**RUN, "head_repository": {"id": 99}}, {**RUN, "pull_requests": []}]:
            self.assertEqual(self.result([run])["tools"][0]["status"], "not_run")
        self.assertEqual(self.result([RUN], total=101)["tools"][0]["status"], "not_run")

    def test_newest_pending_does_not_use_previous_failure(self):
        pending = {**RUN, "id": 26, "status": "in_progress", "conclusion": None}
        self.assertEqual(self.result([RUN, pending])["tools"][0]["status"], "not_run")

    def test_newer_fork_run_without_pr_links_does_not_revive_old_failure(self):
        pending = {**RUN, "id": 26, "status": "in_progress", "conclusion": None, "pull_requests": []}
        self.assertEqual(self.result([RUN, pending])["tools"][0]["status"], "not_run")

    def test_same_repo_historical_shape_without_pr_links_is_unknown(self):
        # The GitHub API may omit PR links even for non-fork workflow runs.
        same_repo_pr = copy.deepcopy(PR)
        same_repo_pr["head"]["repo"]["id"] = same_repo_pr["base"]["repo"]["id"]
        run = {**RUN, "run_attempt": 2, "head_repository": {"id": 12}, "pull_requests": []}
        self.assertEqual(self.result([run], first_pr=same_repo_pr)["tools"][0]["status"], "not_run")
        run["pull_requests"] = [{"number": 42}]
        self.assertEqual(self.result([run], first_pr=same_repo_pr)["tools"][0]["status"], "failed")

    def test_duplicate_run_ids_do_not_select_an_arbitrary_attempt(self):
        pending = {**RUN, "status": "in_progress", "conclusion": None, "run_attempt": 2}
        self.assertEqual(self.result([RUN, pending])["tools"][0]["status"], "not_run")

    def test_fork_runs_with_explicit_different_pr_do_not_contaminate(self):
        other = {**RUN, "id": 26, "pull_requests": [{"number": 43}], "status": "in_progress"}
        self.assertEqual(self.result([RUN, other])["tools"][0]["status"], "failed")

    def test_multiple_linked_prs_do_not_identify_the_trigger(self):
        shared = {**RUN, "pull_requests": [{"number": 42}, {"number": 43}]}
        self.assertEqual(self.result([shared])["tools"][0]["status"], "not_run")
        newer = {**shared, "id": 26}
        self.assertEqual(self.result([RUN, newer])["tools"][0]["status"], "not_run")

    def test_malformed_entries_and_partial_pages_do_not_revive_old_failure(self):
        for entry in [None, {}, {**RUN, "id": 26, "pull_requests": [{"number": True}]}]:
            self.assertEqual(self.result([RUN, entry])["tools"][0]["status"], "not_run")
        self.assertEqual(self.result([RUN], total=2)["tools"][0]["status"], "not_run")

    def test_fork_identity_changes_and_missing_repository_fail_closed(self):
        changed = copy.deepcopy(PR)
        changed["head"]["repo"]["id"] = 99
        with self.assertRaises(ValueError):
            self.result([RUN], second_pr=changed)
        deleted = copy.deepcopy(PR)
        deleted["head"]["repo"] = None
        with self.assertRaises((TypeError, KeyError, ValueError)):
            self.result([RUN], second_pr=deleted)

    def test_changed_head_and_api_error_fail_closed(self):
        changed = copy.deepcopy(PR)
        changed["head"]["sha"] = "b" * 40
        with self.assertRaises(ValueError):
            self.result([RUN], second_pr=changed)
        def broken(_):
            raise OSError("unavailable")
        with self.assertRaises(OSError):
            collect(broken, "owner/repo", "42", SHA)

    def test_main_writes_minimal_manifest_and_refuses_existing_directory(self):
        with tempfile.TemporaryDirectory() as workspace:
            env = {"GITHUB_REPOSITORY": "owner/repo", "PR_NUMBER": "42",
                   "EXPECTED_HEAD_SHA": SHA, "GITHUB_WORKSPACE": workspace}
            with patch.dict(os.environ, env, clear=True), patch("collect_cururu_analysis.gh_api", api_fixture([RUN])):
                main()
                path = Path(workspace) / ".cururu-evidence" / "analysis.json"
                self.assertEqual(json.loads(path.read_text())["tools"][0]["status"], "failed")
                with self.assertRaises(FileExistsError):
                    main()

    def test_github_client_uses_gh_without_token_in_arguments(self):
        response = subprocess.CompletedProcess([], 0, stdout='{"id": 11}', stderr="")
        with patch("collect_cururu_analysis.subprocess.run", return_value=response) as call:
            self.assertEqual(gh_api("repos/owner/repo/pulls/42"), {"id": 11})
        args = call.call_args.args[0]
        self.assertEqual(args, ["gh", "api", "-H", "Accept: application/vnd.github+json", "repos/owner/repo/pulls/42"])
        self.assertEqual(call.call_args.kwargs["timeout"], 30)

    @unittest.skipUnless(os.name == "posix", "executable fake gh requires POSIX")
    def test_process_with_fake_gh_exercises_queries_and_writes_manifest(self):
        with tempfile.TemporaryDirectory() as workspace:
            root = Path(workspace)
            fake = root / "gh"
            fake.write_text(
                f"#!{sys.executable}\n"
                "import json, os, sys\n"
                "p = sys.argv[-1]\n"
                "with open(os.environ['REQUEST_LOG'], 'a') as f: f.write(p + '\\n')\n"
                "if '/pulls/' in p: name = 'pr.json'\n"
                "elif '?event=' in p: name = 'runs.json'\n"
                "else: name = 'workflow.json'\n"
                "with open(os.path.join(os.environ['FIXTURE_DIR'], name)) as f: print(f.read())\n"
            )
            fake.chmod(0o700)
            (root / "pr.json").write_text(json.dumps(PR))
            (root / "workflow.json").write_text(json.dumps({"id": 11, "path": ".github/workflows/cururu-analysis.yml"}))
            (root / "runs.json").write_text(json.dumps({"total_count": 1, "workflow_runs": [RUN]}))
            env = {
                "PATH": f"{root}:{os.defpath}", "GH_TOKEN": "synthetic-token",
                "GITHUB_REPOSITORY": "owner/repo", "PR_NUMBER": "42", "EXPECTED_HEAD_SHA": SHA,
                "GITHUB_WORKSPACE": workspace, "FIXTURE_DIR": workspace, "REQUEST_LOG": str(root / "requests"),
            }
            script = Path(__file__).with_name("collect_cururu_analysis.py").resolve()
            result = subprocess.run([sys.executable, str(script)], env=env, capture_output=True, text=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stderr)
            manifest = json.loads((root / ".cururu-evidence" / "analysis.json").read_text())
            self.assertEqual(manifest["commit_sha"], SHA)
            self.assertEqual(manifest["tools"][0]["status"], "failed")
            requests = (root / "requests").read_text().splitlines()
            self.assertEqual(len(requests), 4)
            self.assertEqual(requests[0], "repos/owner/repo/pulls/42")
            self.assertIn(f"head_sha={SHA}", requests[2])
            self.assertEqual(requests[3], requests[0])
            self.assertNotIn("synthetic-token", result.stdout + result.stderr + "\n".join(requests))

    @unittest.skipUnless(os.name == "posix", "executable fake gh requires POSIX")
    def test_process_fails_without_manifest_when_pr_head_changes(self):
        with tempfile.TemporaryDirectory() as workspace:
            root = Path(workspace)
            fake = root / "gh"
            fake.write_text(
                f"#!{sys.executable}\n"
                "import json, os, sys\n"
                "p = sys.argv[-1]; root = os.environ['FIXTURE_DIR']\n"
                "if '/pulls/' in p:\n"
                " marker = os.path.join(root, 'visited')\n"
                " name = 'pr2.json' if os.path.exists(marker) else 'pr.json'\n"
                " open(marker, 'a').close()\n"
                "elif '?event=' in p: name = 'runs.json'\n"
                "else: name = 'workflow.json'\n"
                "with open(os.path.join(root, name)) as f: print(f.read())\n"
            )
            fake.chmod(0o700)
            changed = copy.deepcopy(PR)
            changed["head"]["sha"] = "b" * 40
            for name, value in [("pr.json", PR), ("pr2.json", changed),
                                ("workflow.json", {"id": 11, "path": ".github/workflows/cururu-analysis.yml"}),
                                ("runs.json", {"total_count": 1, "workflow_runs": [RUN]})]:
                (root / name).write_text(json.dumps(value))
            env = {"PATH": f"{root}:{os.defpath}", "GH_TOKEN": "synthetic-token",
                   "GITHUB_REPOSITORY": "owner/repo", "PR_NUMBER": "42", "EXPECTED_HEAD_SHA": SHA,
                   "GITHUB_WORKSPACE": workspace, "FIXTURE_DIR": workspace}
            script = Path(__file__).with_name("collect_cururu_analysis.py").resolve()
            result = subprocess.run([sys.executable, str(script)], env=env, capture_output=True, text=True, timeout=15)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / ".cururu-evidence" / "analysis.json").exists())


if __name__ == "__main__":
    unittest.main()
