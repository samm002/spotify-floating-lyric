#!/usr/bin/env python3
"""Install pinned upstream Codex templates without invoking upstream init."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
SHARED_HELPER = '"$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")/.codex/scripts/goal-git.sh"'


def adapt_hook_commands(contents: bytes) -> bytes:
    hooks = json.loads(contents)
    changed = 0
    marker = ".codex/scripts/goal-git.sh"
    for registrations in hooks.get("hooks", {}).values():
        for registration in registrations:
            for hook in registration.get("hooks", []):
                command = hook.get("command", "")
                if marker not in command:
                    continue
                if command.count(marker) != 1:
                    raise ValueError("Unexpected repeated helper in upstream hook command")
                suffix = command[command.index(marker) + len(marker):].lstrip(" \t\"")
                if not suffix.startswith(("harness hook", "harness stop-check")):
                    raise ValueError("Unexpected upstream goal-git hook command")
                hook["command"] = f"{SHARED_HELPER} {suffix}"
                changed += 1
    if not changed:
        raise ValueError("No upstream goal-git hook commands found")
    return (json.dumps(hooks, indent=2) + "\n").encode()


def prepare_baseline(archive: Path) -> dict[Path, bytes]:
    lock = json.loads((ROOT / "harness/upstream.lock.json").read_text())
    if hashlib.sha256(archive.read_bytes()).hexdigest() != lock["archive_sha256"]:
        raise ValueError("Archive checksum differs from the reviewed upstream lock")

    candidates = {}
    with tarfile.open(archive) as source:
        roots = {PurePosixPath(m.name).parts[0] for m in source if m.name}
        if len(roots) != 1:
            raise ValueError("Expected one archive root")
        prefix = f"{roots.pop()}/{lock['template']}/"
        for member in source.getmembers():
            if not member.name.startswith(prefix) or not member.isfile():
                continue
            relative = PurePosixPath(member.name[len(prefix):])
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError("Unexpected template path")
            if not relative.parts or relative.parts[0] not in {".agents", ".codex"}:
                continue
            stream = source.extractfile(member)
            if stream is None:
                raise ValueError(f"Cannot read {member.name}")
            candidates[ROOT / str(relative)] = stream.read()

    config = ROOT / ".codex/config.toml"
    text = candidates[config].decode()
    for setting in ('approval_policy = "never"\n', 'sandbox_mode = "danger-full-access"\n'):
        if text.count(setting) != 1:
            raise ValueError(f"Unexpected upstream setting: {setting.strip()}")
        text = text.replace(setting, "", 1)
    hooks_enabled = lock.get("hooks_enabled", False)
    if not isinstance(hooks_enabled, bool):
        raise ValueError("hooks_enabled must be a boolean")
    if text.count("hooks = true\n") != 1:
        raise ValueError("Unexpected upstream hooks setting")
    text = text.replace("hooks = true\n", f"hooks = {str(hooks_enabled).lower()}\n", 1)
    text = text.replace("max_concurrent_threads_per_session = 8", "max_concurrent_threads_per_session = 4")
    text = text.replace('default_subagent_model = "gpt-5.6-terra"', f'default_subagent_model = "{lock["model"]}"')
    orchestrator = lock.get("orchestrator", {})
    root_settings = "".join(f"{key} = {json.dumps(orchestrator[key])}\n" for key in ("model", "model_reasoning_effort") if key in orchestrator)
    text = "# Project bootstrap: inherit main-session permission controls.\n" + root_settings + text
    tomllib.loads(text)
    candidates[config] = text.encode()

    hooks = ROOT / ".codex/hooks.json"
    candidates[hooks] = adapt_hook_commands(candidates[hooks])

    models = ROOT / ".codex/goal-models.json"
    routing = json.loads(candidates[models])
    routing["$capabilities"]["vision_models"] = [lock["model"]]
    for levels in routing["$routing"].values():
        for route in levels.values():
            if route is not None:
                route["model"] = lock["model"]
    for role, levels in lock.get("role_overrides", {}).items():
        for level, override in levels.items():
            route = routing["$routing"][role][level]
            if route is None:
                raise ValueError(f"Cannot override skipped route: {role}/{level}")
            route.update(override)
    candidates[models] = (json.dumps(routing, indent=2) + "\n").encode()

    for path, contents in list(candidates.items()):
        if path.parent == ROOT / ".codex/agents" and path.suffix == ".toml":
            role = tomllib.loads(contents.decode())
            sandbox = "workspace-write" if path.stem in {"builder", "builder-expert"} else "read-only"
            setting = f'sandbox_mode = "{role["sandbox_mode"]}"'
            contents = contents.decode().replace(setting, f'sandbox_mode = "{sandbox}"', 1)
            tomllib.loads(contents)
            candidates[path] = contents.encode()
    return candidates


def template_path(value: str) -> Path:
    relative = PurePosixPath(value)
    if relative.is_absolute() or ".." in relative.parts or not relative.parts or relative.parts[0] not in {".agents", ".codex"}:
        raise ValueError(f"Unexpected patch target: {value}")
    return Path(str(relative))


def prepare(archive: Path) -> dict[Path, bytes]:
    candidates = prepare_baseline(archive)
    lock = json.loads((ROOT / "harness/upstream.lock.json").read_text())
    for item in lock.get("patches", []):
        patch_relative = PurePosixPath(item["path"])
        if patch_relative.is_absolute() or ".." in patch_relative.parts or patch_relative.parts[:2] != ("harness", "patches"):
            raise ValueError("Unexpected adapter patch path")
        patch = (ROOT / str(patch_relative)).read_bytes()
        if hashlib.sha256(patch).hexdigest() != item["sha256"]:
            raise ValueError("Adapter patch checksum differs from the reviewed lock")
        for name, digest in item["inputs"].items():
            path = ROOT / template_path(name)
            if path not in candidates or hashlib.sha256(candidates[path]).hexdigest() != digest:
                raise ValueError(f"Unexpected upstream patch input: {name}")
        # Validate every target before invoking patch, even though its bytes are pinned.
        for line in patch.decode().splitlines():
            if line.startswith(("--- ", "+++ ")):
                name = line[4:].split("\t", 1)[0]
                if name != "/dev/null":
                    if not name.startswith(("a/", "b/")):
                        raise ValueError("Unexpected unified patch path")
                    template_path(name[2:])
        with tempfile.TemporaryDirectory(prefix="spotify-harness-patch-") as directory:
            stage = Path(directory)
            for path, contents in candidates.items():
                target = stage / path.relative_to(ROOT)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(contents)
            result = subprocess.run(
                ["patch", "--batch", "--forward", "--fuzz=0", "--no-backup-if-mismatch", "-p1", "-d", str(stage)],
                input=patch, capture_output=True,
            )
            if result.returncode:
                raise ValueError("Reviewed adapter patch did not apply exactly: " + result.stderr.decode() + result.stdout.decode())
            updated = {ROOT / p.relative_to(stage): p.read_bytes() for p in stage.rglob("*") if p.is_file()}
        changed = {str(p.relative_to(ROOT)) for p in candidates.keys() | updated.keys() if candidates.get(p) != updated.get(p)}
        if changed != set(item["outputs"]):
            raise ValueError("Adapter changed unexpected template files")
        for name, digest in item["outputs"].items():
            path = ROOT / template_path(name)
            if path not in updated or hashlib.sha256(updated[path]).hexdigest() != digest:
                raise ValueError(f"Unexpected adapter output: {name}")
        candidates = updated
    return candidates


def refuse_symlink_destination(path: Path) -> None:
    if path.is_symlink() or any(p.is_symlink() for p in path.parents if p != ROOT):
        raise ValueError(f"Refusing symlink destination: {path}")


def cache_archive(archive: Path) -> None:
    digest = json.loads((ROOT / "harness/upstream.lock.json").read_text())["archive_sha256"]
    contents = archive.read_bytes()
    if hashlib.sha256(contents).hexdigest() != digest:
        raise ValueError("Archive changed before caching")
    destination = ROOT / ".tools/cache" / f"upstream-{digest}.tar.gz"
    refuse_symlink_destination(destination)
    if destination.exists():
        if not destination.is_file() or destination.read_bytes() != contents:
            raise ValueError("Existing archive cache differs from the reviewed input")
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    # A unique temporary file avoids following a pre-existing cache-temp symlink.
    with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=".upstream-", delete=False) as output:
        temporary = Path(output.name)
        output.write(contents)
    try:
        refuse_symlink_destination(destination)
        temporary.replace(destination)
    finally:
        temporary.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--check", action="store_true", help="Validate planned files without writing")
    parser.add_argument("--upgrade-reviewed", action="store_true", help="Also accept exact normalized upstream files as upgrade inputs")
    args = parser.parse_args()
    try:
        candidates = prepare(args.archive)
        baseline = prepare_baseline(args.archive) if args.upgrade_reviewed else {}
        for path, contents in candidates.items():
            # Refuse overwriting a project-specific customization on reinstall.
            refuse_symlink_destination(path)
            if path.exists() and (not path.is_file() or (path.read_bytes() != contents and path.read_bytes() != baseline.get(path))):
                raise ValueError(f"Refusing to overwrite changed destination: {path}")
        if args.check:
            print(f"Validated {len(candidates)} planned template files; no files written")
            return
        for path, contents in candidates.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(contents)
            if path.suffix == ".sh":
                path.chmod(0o755)
        cache_archive(args.archive)
        print(f"Installed {len(candidates)} project template files; no goal or global trust changes")
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        parser.exit(1, f"BLOCKED: {error}\n")


if __name__ == "__main__":
    main()
