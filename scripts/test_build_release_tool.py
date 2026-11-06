import hashlib
import importlib.util
import io
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "build_release_tool", Path(__file__).with_name("build-release-tool.py")
)
tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool)


class ReleaseToolTests(unittest.TestCase):
    def test_git_cliff_uses_reviewed_no_default_feature_graph(self):
        version, checksum, flags = tool.TOOLS["git-cliff"]
        self.assertEqual(version, "2.14.2")
        self.assertEqual(len(checksum), 64)
        self.assertEqual(flags, ["--no-default-features"])
        self.assertTrue((tool.ROOT / "tools" / "locks" / f"git-cliff-{version}.lock").is_file())

    def test_valid_archive_build_uses_reviewed_lock_and_feature_flags(self):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
            content = b"upstream lock"
            entry = tarfile.TarInfo("test-tool-1.0/Cargo.lock")
            entry.size = len(content)
            archive.addfile(entry, io.BytesIO(content))
        data = buffer.getvalue()
        checksum = hashlib.sha256(data).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            locks = root / "tools" / "locks"
            locks.mkdir(parents=True)
            (locks / "test-tool-1.0.lock").write_bytes(b"reviewed lock")
            install_root = root / "install"
            observed = []

            def capture_install(command, *, check):
                self.assertTrue(check)
                source = Path(command[command.index("--path") + 1])
                observed.append((command, (source / "Cargo.lock").read_bytes()))

            with patch.object(tool, "ROOT", root), \
                 patch.object(tool, "TOOLS", {"test-tool": ("1.0", checksum, ["--no-default-features"])}), \
                 patch.object(tool.urllib.request, "urlopen", return_value=io.BytesIO(data)), \
                 patch.object(tool.subprocess, "run", side_effect=capture_install), \
                 patch.object(sys, "argv", ["builder", "test-tool", "--root", str(install_root)]):
                tool.main()

            self.assertEqual(len(observed), 1)
            command, lock_content = observed[0]
            self.assertEqual(lock_content, b"reviewed lock")
            self.assertEqual(command[:2], ["cargo", "install"])
            self.assertIn("--locked", command)
            self.assertIn("--no-default-features", command)
            self.assertEqual(command[command.index("--root") + 1], str(install_root.resolve()))

    def test_git_cliff_profiler_transform_is_exact_and_fails_closed(self):
        original = ('[features]\nprofiler = [\n    "dep:pprof",\n    "dep:rand",\n'
                    '    "profiler-flamegraph",\n]\nprofiler-flamegraph = ["pprof/flamegraph"]\n'
                    '\n[dependencies.pprof]\nversion = "0.15.0"\noptional = true\n'
                    '\n[dependencies.other]\nversion = "1"\n')
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text(original, encoding="utf-8")
            tool.remove_git_cliff_profiler(manifest)
            changed = manifest.read_text(encoding="utf-8")
            self.assertNotIn("[dependencies.pprof]", changed)
            self.assertNotIn('"pprof/flamegraph"', changed)
            self.assertIn('[dependencies.other]\nversion = "1"', changed)
            with self.assertRaises(ValueError):
                tool.remove_git_cliff_profiler(manifest)
            self.assertEqual(manifest.read_text(encoding="utf-8"), changed)
            for invalid in (original.replace('version = "0.15.0"', 'version = "0.16.0"'), original + original):
                manifest.write_text(invalid, encoding="utf-8")
                with self.assertRaises(ValueError):
                    tool.remove_git_cliff_profiler(manifest)
                self.assertEqual(manifest.read_text(encoding="utf-8"), invalid)

    def run_with_archive(self, archive, checksum):
        class Response(io.BytesIO):
            def __enter__(self):
                return self

            def __exit__(self, *args):
                self.close()

        with patch.object(tool, "TOOLS", {"test-tool": ("1.0", checksum, [])}):
            with patch.object(tool.urllib.request, "urlopen", return_value=Response(archive)):
                with patch.object(sys, "argv", ["builder", "test-tool", "--root", "unused"]):
                    with self.assertRaises(ValueError):
                        tool.main()

    def test_rejects_incorrect_checksum(self):
        self.run_with_archive(b"not an archive", "0" * 64)

    def test_rejects_archive_path_traversal(self):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
            entry = tarfile.TarInfo("test-tool-1.0/../../outside")
            entry.size = 1
            archive.addfile(entry, io.BytesIO(b"x"))
        data = buffer.getvalue()
        self.run_with_archive(data, hashlib.sha256(data).hexdigest())


if __name__ == "__main__":
    unittest.main()
