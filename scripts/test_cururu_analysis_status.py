import unittest

from cururu_analysis_status import classify_run


SHA = "a" * 40
IDENTITY = dict(expected_sha=SHA, workflow_id=11, repository_id=12, head_repository_id=13)


def fixture():
    return {
        "id": 25,
        "run_attempt": 1,
        "event": "pull_request",
        "workflow_id": 11,
        "repository": {"id": 12},
        "head_repository": {"id": 13},
        "head_sha": SHA,
        "status": "completed",
        "conclusion": "success",
    }


class AnalysisStatusTests(unittest.TestCase):
    def test_success_and_failure_are_distinct(self):
        run = fixture()
        self.assertEqual(classify_run(run, **IDENTITY), "passed")
        run["conclusion"] = "failure"
        self.assertEqual(classify_run(run, **IDENTITY), "failed")

    def test_other_revisions_and_origins_are_unknown(self):
        for key, value in [("head_sha", "b" * 40), ("workflow_id", 99),
                           ("repository", {"id": 99}), ("head_repository", {"id": 99}),
                           ("event", "push"), ("id", True), ("run_attempt", 0)]:
            run = fixture()
            run[key] = value
            self.assertEqual(classify_run(run, **IDENTITY), "unknown", key)

    def test_pending_cancelled_and_malformed_are_unknown(self):
        for status, conclusion in [("in_progress", None), ("queued", None),
                                   ("completed", "cancelled"), ("completed", None),
                                   ("completed", "skipped")]:
            run = fixture()
            run.update(status=status, conclusion=conclusion)
            self.assertEqual(classify_run(run, **IDENTITY), "unknown")
        self.assertEqual(classify_run({}, **IDENTITY), "unknown")
        self.assertEqual(classify_run(fixture(), **{**IDENTITY, "expected_sha": "bad"}), "unknown")


if __name__ == "__main__":
    unittest.main()
