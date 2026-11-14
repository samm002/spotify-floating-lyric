#!/usr/bin/env python3
"""Apply the reviewed project policy through the upstream configuration helper."""

import argparse
import json
import os
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]
BASE_KEYS = (
    "goal_source", "target_branch", "platform", "concurrency", "auto_merge",
    "review_mode", "review_max_iterations", "max_rework", "max_escalations",
    "max_verify_retries", "visual_mode",
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Show proposed configuration without writing")
    args = parser.parse_args()
    try:
        policy = json.loads((ROOT / "harness/project-policy.json").read_text())
        policy["verify_commands"] = json.loads((ROOT / "harness/verify-commands.json").read_text())
        if args.check:
            print(json.dumps(policy, indent=2))
            return
        if (ROOT / "state.json").exists():
            raise ValueError("Setup refuses existing goal state; use explicit helper config commands for an existing workflow")
        helper = ROOT / ".codex/scripts/goal-git.sh"
        if not helper.is_file():
            raise ValueError("Install the reviewed harness before configuring it")
        env = os.environ.copy()
        env["PATH"] = str(ROOT / ".tools/bin") + os.pathsep + env.get("PATH", "")
        for key in ("GOAL_ID", "GOAL_RUN_ID", "GOAL_ISSUE", "GOAL_GROUP", "GOAL_TASK", "GOAL_REPO", "GOAL_ISSUE_REPO"):
            env[key] = ""
        base = [str(policy[key]).lower() if isinstance(policy[key], bool) else str(policy[key]) for key in BASE_KEYS]
        subprocess.run([str(helper), "config", "set", *base], cwd=ROOT, env=env, check=True, stdout=subprocess.DEVNULL)
        for key, value in policy.items():
            if key not in BASE_KEYS:
                subprocess.run([str(helper), "config", "put", key, json.dumps(value)], cwd=ROOT, env=env, check=True, stdout=subprocess.DEVNULL)
        subprocess.run([str(helper), "config", "get"], cwd=ROOT, env=env, check=True)
    except (OSError, KeyError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"BLOCKED: {error}\n")


if __name__ == "__main__":
    main()
