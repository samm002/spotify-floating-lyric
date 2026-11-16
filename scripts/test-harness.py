#!/usr/bin/env python3
"""Run offline harness regressions using the shared workflow root, including from worktrees."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


def resolve_inputs(checkout: Path) -> tuple[Path, Path]:
    helper = os.environ.get("GOAL_GIT")
    if helper:
        path = Path(helper).resolve()
        if path.name != "goal-git.sh" or path.parent.name != "scripts" or path.parent.parent.name != ".codex":
            raise ValueError("GOAL_GIT does not identify the project-root helper")
        workflow = path.parents[2]
    else:
        result = subprocess.run(
            ["git", "-C", str(checkout), "rev-parse", "--git-common-dir"],
            check=True, capture_output=True, text=True,
        )
        common = Path(result.stdout.strip())
        common = (checkout / common).resolve() if not common.is_absolute() else common.resolve()
        if common.name != ".git":
            raise ValueError("Cannot infer shared workflow root from this Git layout; export GOAL_GIT")
        workflow = common.parent
    if not (workflow / ".codex/scripts/goal-git.sh").is_file():
        raise ValueError("Shared harness is missing; install it before running harness tests")
    lock = json.loads((checkout / "harness/upstream.lock.json").read_text())
    digest = lock["archive_sha256"]
    archive = workflow / ".tools/cache" / f"upstream-{digest}.tar.gz"
    if not archive.is_file():
        raise ValueError("Pinned archive cache is missing; rerun the reviewed harness installer")
    if hashlib.sha256(archive.read_bytes()).hexdigest() != digest:
        raise ValueError("Pinned archive cache checksum mismatch")
    return workflow, archive


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Report resolved inputs without running tests")
    args = parser.parse_args()
    checkout = Path(__file__).resolve().parents[1]
    try:
        workflow, archive = resolve_inputs(checkout)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"BLOCKED: {error}", file=sys.stderr)
        return 2
    if args.check:
        print(json.dumps({"HARNESS_SOURCE": str(workflow), "HARNESS_ARCHIVE": str(archive)}))
        return 0
    environment = dict(os.environ, HARNESS_SOURCE=str(workflow), HARNESS_ARCHIVE=str(archive))
    return subprocess.call(
        [sys.executable, "-m", "unittest", "discover", "-s", str(checkout / "tests/harness"), "-v"],
        cwd=checkout, env=environment,
    )


if __name__ == "__main__":
    raise SystemExit(main())
