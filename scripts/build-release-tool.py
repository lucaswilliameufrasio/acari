#!/usr/bin/env python3
"""Build a release tool from its verified crates.io archive and audited lockfile."""

import argparse
import hashlib
import io
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
TOOLS = {
    "cargo-about": ("0.9.3", "f2309e4e7cd6fbb25c4c8d68e69c728936dec58a0cb7940dcb382d5be32aa52b", ["--features", "cli"]),
    "cargo-dist": ("0.34.0", "063681fbc91a26fae344773db3830a5f35bf9ca626a254f83bd826c3bf636844", []),
    "git-cliff": ("2.14.2", "3cf0865d861e5b64a73ca527c62c1c566412ec1cc5a4ad5c5692650bfb0e2a79", ["--no-default-features"]),
}

GIT_CLIFF_PROFILER_FEATURES = '''profiler = [
    "dep:pprof",
    "dep:rand",
    "profiler-flamegraph",
]
profiler-flamegraph = ["pprof/flamegraph"]'''
GIT_CLIFF_PPROF_DEPENDENCY = '''[dependencies.pprof]
version = "0.15.0"
optional = true
'''


def remove_git_cliff_profiler(manifest):
    """Transform the verified 2.14.2 manifest; fail closed if upstream changes."""
    text = manifest.read_text(encoding="utf-8")
    if text.count(GIT_CLIFF_PROFILER_FEATURES) != 1 or text.count(GIT_CLIFF_PPROF_DEPENDENCY) != 1:
        raise ValueError("Unexpected git-cliff profiler manifest")
    text = text.replace(
        GIT_CLIFF_PROFILER_FEATURES,
        'profiler = ["dep:rand", "profiler-flamegraph"]\nprofiler-flamegraph = []',
    ).replace(GIT_CLIFF_PPROF_DEPENDENCY, "")
    manifest.write_text(text, encoding="utf-8", newline="\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tool", choices=TOOLS)
    parser.add_argument("--root", required=True, type=Path, help="Cargo installation root")
    args = parser.parse_args()
    version, checksum, features = TOOLS[args.tool]
    name = f"{args.tool}-{version}"
    lock = ROOT / "tools" / "locks" / f"{name}.lock"
    url = f"https://static.crates.io/crates/{args.tool}/{name}.crate"
    request = urllib.request.Request(url, headers={"User-Agent": "acari-release-tool-builder"})
    with urllib.request.urlopen(request, timeout=60) as response:
        archive = response.read(32 * 1024 * 1024 + 1)
    if len(archive) > 32 * 1024 * 1024 or hashlib.sha256(archive).hexdigest() != checksum:
        raise ValueError(f"Invalid archive size or checksum for {name}")

    with tempfile.TemporaryDirectory(prefix="acari-release-tool-") as temp:
        base = Path(temp)
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as tar:
            for member in tar.getmembers():
                path = PurePosixPath(member.name)
                if (not path.parts or path.parts[0] != name or ".." in path.parts
                        or path.is_absolute() or "\\" in member.name or ":" in member.name):
                    raise ValueError(f"Invalid archive path: {member.name}")
                if not member.isfile() and not member.isdir():
                    raise ValueError(f"Unsupported archive entry: {member.name}")
                destination = base.joinpath(*path.parts)
                if member.isdir():
                    destination.mkdir(parents=True, exist_ok=True)
                else:
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    with tar.extractfile(member) as source, destination.open("wb") as target:
                        shutil.copyfileobj(source, target)
        source_dir = base / name
        if args.tool == "git-cliff":
            if version != "2.14.2" or features != ["--no-default-features"]:
                raise ValueError("Unreviewed git-cliff version or features")
            remove_git_cliff_profiler(source_dir / "Cargo.toml")
        shutil.copyfile(lock, source_dir / "Cargo.lock")
        subprocess.run(
            ["cargo", "install", "--path", os.fspath(source_dir), "--locked", "--root", os.fspath(args.root.resolve()), *features],
            check=True,
        )


if __name__ == "__main__":
    main()
