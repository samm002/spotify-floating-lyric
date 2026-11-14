"""Regression checks for pinned template patching and reviewed upgrades."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import sys
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

PROJECT = Path(__file__).resolve().parents[2]
ARCHIVE = Path(os.environ.get('HARNESS_ARCHIVE', '/nonexistent-unless-explicitly-provided'))


class InstallerFixture(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not ARCHIVE.is_file():
            raise RuntimeError('Pinned archive is missing; run scripts/test-harness.py after installing the harness')

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='spotify-installer-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'project'; self.root.mkdir()
        (self.root / 'harness/patches').mkdir(parents=True)
        shutil.copy2(PROJECT / 'harness/upstream.lock.json', self.root / 'harness/upstream.lock.json')
        for path in (PROJECT / 'harness/patches').glob('*.patch'):
            shutil.copy2(path, self.root / 'harness/patches' / path.name)
        spec = importlib.util.spec_from_file_location('harness_installer_fixture', PROJECT / 'scripts/install-harness.py')
        self.module = importlib.util.module_from_spec(spec); spec.loader.exec_module(self.module)
        self.module.ROOT = self.root

    def run_install(self, *flags, fails=False):
        with patch.object(sys, 'argv', ['install-harness.py', str(ARCHIVE), *flags]), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            if fails:
                with self.assertRaises(SystemExit) as raised: self.module.main()
                self.assertEqual(raised.exception.code, 1)
            else:
                self.module.main()

    def write_baseline(self):
        for path, data in self.module.prepare_baseline(ARCHIVE).items():
            path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(data)

    def test_fresh_install_and_repeat_reproduce_pinned_patch(self):
        self.run_install('--check')
        self.assertFalse((self.root / '.codex').exists())
        self.run_install()
        expected = self.module.prepare(ARCHIVE)
        self.assertEqual(len(expected), 33)
        self.assertTrue(all(path.read_bytes() == data for path, data in expected.items()))
        self.run_install('--check'); self.run_install()
        source = Path(os.environ.get('HARNESS_SOURCE', PROJECT))
        self.assertTrue(all((source / p.relative_to(self.root)).read_bytes() == data for p, data in expected.items()))

    def test_source_lock_controls_models_permissions_and_hook_setting(self):
        planned = self.module.prepare(ARCHIVE)
        config = tomllib.loads(planned[self.root / '.codex/config.toml'].decode())
        self.assertEqual(config['model'], 'gpt-6.1-sol')
        self.assertEqual(config['model_reasoning_effort'], 'high')
        self.assertTrue(config['features']['hooks'])
        self.assertNotIn('approval_policy', config)
        self.assertNotIn('sandbox_mode', config)

        routing = json.loads(planned[self.root / '.codex/goal-models.json'])['$routing']
        for level in ('TRIVIAL', 'NORMAL', 'COMPLEX', 'ARCHITECTURAL'):
            self.assertEqual(routing['builder'][level]['model'], 'gpt-6-luna')
            self.assertEqual(routing['builder'][level]['model_reasoning_effort'], 'xhigh')
            self.assertEqual(routing['researcher'][level]['model'], 'gpt-6.1-sol')
            self.assertEqual(routing['researcher'][level]['model_reasoning_effort'], 'low')
            self.assertEqual(routing['visual-reviewer'][level]['model'], 'gpt-6.1-sol')
            self.assertEqual(routing['visual-reviewer'][level]['model_reasoning_effort'], 'low')

        for role, expected in {
            'builder': 'workspace-write', 'builder-expert': 'workspace-write',
            'planner': 'read-only', 'researcher': 'read-only', 'reviewer': 'read-only',
            'visual-reviewer': 'read-only',
        }.items():
            role_path = self.root / f'.codex/agents/{role}.toml'
            self.assertEqual(tomllib.loads(planned[role_path].decode())['sandbox_mode'], expected)

        lock_path = self.root / 'harness/upstream.lock.json'
        lock = json.loads(lock_path.read_text())
        self.assertIs(lock['hooks_enabled'], True)
        lock['hooks_enabled'] = False
        lock_path.write_text(json.dumps(lock))
        disabled = self.module.prepare_baseline(ARCHIVE)
        disabled_config = tomllib.loads(disabled[self.root / '.codex/config.toml'].decode())
        self.assertFalse(disabled_config['features']['hooks'])

    def test_hook_commands_resolve_from_subdirectories_and_linked_worktrees(self):
        planned = self.module.prepare(ARCHIVE)
        hooks = json.loads(planned[self.root / '.codex/hooks.json'])['hooks']
        start = hooks['SubagentStart'][0]['hooks'][0]
        stop = hooks['SubagentStop'][0]['hooks'][0]
        stop_check = hooks['Stop'][0]['hooks'][0]
        self.assertTrue(start['async'])
        self.assertEqual(start['timeout'], 10)
        self.assertFalse(stop['async'])
        self.assertEqual(stop['timeout'], 15)
        self.assertEqual(stop_check['timeout'], 15)
        self.assertIn('2>/dev/null || true', stop_check['command'])
        for hook in (start, stop, stop_check):
            self.assertIn('git rev-parse --path-format=absolute --git-common-dir', hook['command'])
            self.assertIn('dirname', hook['command'])
            self.assertNotIn('git rev-parse --show-toplevel', hook['command'])

        main = Path(self.temp.name) / 'main checkout with spaces'
        worktree = Path(self.temp.name) / 'linked checkout with spaces'
        main.mkdir()
        run = lambda *args: subprocess.run(args, check=True, capture_output=True, text=True)
        run('git', '-C', str(main), 'init', '-q')
        run('git', '-C', str(main), 'config', 'user.name', 'Installer Fixture')
        run('git', '-C', str(main), 'config', 'user.email', 'installer@example.test')
        (main / 'README').write_text('fixture\n')
        run('git', '-C', str(main), 'add', 'README')
        run('git', '-C', str(main), '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')
        run('git', '-C', str(main), 'worktree', 'add', '-q', '-b', 'task-worktree', str(worktree), 'HEAD')

        helper = main / '.codex/scripts/goal-git.sh'
        helper.parent.mkdir(parents=True)
        helper.write_text(
            '#!/bin/sh\nprintf \'%s\\t%s\\n\' "$*" "$PWD" >> "$HOOK_LOG"\n'
            'if [ "$*" = "harness stop-check" ]; then printf \'fixture diagnostic\\n\' >&2; exit 7; fi\n'
        )
        helper.chmod(0o755)
        directories = [main / 'src/nested', worktree / 'src/nested']
        for directory in directories:
            directory.mkdir(parents=True)
        log = Path(self.temp.name) / 'hook calls.log'
        environment = dict(os.environ, HOOK_LOG=str(log))
        for directory in directories:
            for command in (start['command'], stop['command'], stop_check['command']):
                result = subprocess.run(['bash', '-c', command], cwd=directory, env=environment, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
                self.assertEqual(result.stderr, '')

        calls = log.read_text().splitlines()
        self.assertEqual(len(calls), 6)
        for directory in directories:
            self.assertEqual(calls.count(f'harness hook\t{directory}'), 2)
            self.assertEqual(calls.count(f'harness stop-check\t{directory}'), 1)

    def test_exact_baseline_requires_explicit_upgrade(self):
        self.write_baseline()
        self.run_install('--check', fails=True)
        self.run_install('--upgrade-reviewed', '--check')
        self.run_install('--upgrade-reviewed')
        self.run_install('--check')

    def test_customization_is_never_overwritten(self):
        self.write_baseline()
        changed = self.root / '.codex/scripts/goal-delegation.sh'
        changed.write_text('user customization\n')
        self.run_install('--upgrade-reviewed', fails=True)
        self.assertEqual(changed.read_text(), 'user customization\n')
        self.assertFalse((self.root / '.agents/skills/goal/references/lifecycle.md').exists())

    def test_bad_patch_checksum_fails_before_destination_writes(self):
        path = next((self.root / 'harness/patches').glob('*.patch'))
        path.write_text(path.read_text() + '\nchanged\n')
        self.run_install(fails=True)
        self.assertFalse((self.root / '.codex').exists())

    def test_expected_input_hash_and_archive_checksum_are_required(self):
        path = self.root / 'harness/upstream.lock.json'; lock = json.loads(path.read_text())
        key = next(iter(lock['patches'][0]['inputs']))
        lock['patches'][0]['inputs'][key] = '0' * 64; path.write_text(json.dumps(lock))
        self.run_install(fails=True)
        self.assertFalse((self.root / '.codex').exists())
        lock['archive_sha256'] = '0' * 64; path.write_text(json.dumps(lock))
        self.run_install(fails=True)

    def test_symlink_destinations_are_refused(self):
        outside = Path(self.temp.name) / 'outside'; outside.mkdir()
        (self.root / '.codex').symlink_to(outside, target_is_directory=True)
        self.run_install('--upgrade-reviewed', fails=True)
        self.assertEqual(list(outside.iterdir()), [])

    def test_archive_cache_is_verified_and_symlink_safe(self):
        self.run_install()
        digest=json.loads((self.root/'harness/upstream.lock.json').read_text())['archive_sha256']
        self.assertEqual((self.root/'.tools/cache'/f'upstream-{digest}.tar.gz').read_bytes(),ARCHIVE.read_bytes())
        outside=Path(self.temp.name)/'external-cache'; outside.mkdir()
        shutil.rmtree(self.root/'.tools/cache')
        (self.root/'.tools/cache').symlink_to(outside,target_is_directory=True)
        self.run_install(fails=True)
        self.assertEqual(list(outside.iterdir()),[])


if __name__ == '__main__':
    unittest.main()
