import os
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).with_name("resolve-cururu-head.sh").resolve()


class ResolveHeadTests(unittest.TestCase):
    def resolve(self, event, head="", number="42", api_sha="b" * 40, api_exit=0):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            gh = root / "gh"
            gh.write_text(f"#!/bin/sh\nprintf '%s\\n' '{api_sha}'\nexit {api_exit}\n")
            gh.chmod(0o700)
            output = root / "output"
            output.touch()
            env = {
                "PATH": f"{root}:{os.defpath}",
                "EVENT_NAME": event,
                "EVENT_HEAD_SHA": head,
                "PR_NUMBER": number,
                "GITHUB_REPOSITORY": "owner/repo",
                "GITHUB_OUTPUT": str(output),
            }
            result = subprocess.run(["sh", str(SCRIPT)], env=env, capture_output=True, text=True)
            return result.returncode, output.read_text()

    def test_pr_event_uses_event_snapshot(self):
        self.assertEqual(self.resolve("pull_request_target", "a" * 40), (0, f"sha={'a' * 40}\n"))

    def test_comment_resolves_head(self):
        self.assertEqual(self.resolve("issue_comment"), (0, f"sha={'b' * 40}\n"))

    def test_comment_queries_the_expected_repository_and_pr(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            gh = root / "gh"
            gh.write_text("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$CALL_LOG\"\nprintf '%s\\n' \"$FIXTURE_SHA\"\n")
            gh.chmod(0o700)
            output = root / "output"
            output.touch()
            log = root / "call"
            env = {
                "PATH": f"{root}:{os.defpath}",
                "EVENT_NAME": "issue_comment",
                "PR_NUMBER": "42",
                "GITHUB_REPOSITORY": "owner/repo",
                "GITHUB_OUTPUT": str(output),
                "CALL_LOG": str(log),
                "FIXTURE_SHA": "c" * 40,
            }
            subprocess.run(["sh", str(SCRIPT)], env=env, check=True, capture_output=True)
            self.assertEqual(log.read_text().splitlines(), ["api", "repos/owner/repo/pulls/42", "--jq", ".head.sha"])
            self.assertEqual(output.read_text(), f"sha={'c' * 40}\n")

    def test_invalid_values_fail_closed(self):
        for args in [
            ("pull_request_target", "bad"),
            ("pull_request_target", "A" * 40),
            ("pull_request_target", "a" * 39),
            ("issue_comment", "", "42;echo"),
            ("issue_comment", "", "42", "not-a-sha"),
            ("issue_comment", "", "42", "b" * 40, 1),
            ("other",),
        ]:
            code, output = self.resolve(*args)
            self.assertNotEqual(code, 0)
            self.assertEqual(output, "")


if __name__ == "__main__":
    unittest.main()
