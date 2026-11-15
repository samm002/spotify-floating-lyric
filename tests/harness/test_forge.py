"""Exercise the pinned forge adapter without contacting a forge service."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

PROJECT = Path(__file__).resolve().parents[2]
ARCHIVE = Path(os.environ.get('HARNESS_ARCHIVE', '/nonexistent-unless-explicitly-provided'))

MOCK_FORGE = r'''source "$FORGE_SCRIPT"
forge_require_flags() { :; }
forge_run() {
  local mode="$1" cli="$2" endpoint page arg previous="" response
  shift 2
  printf '%s %s\n' "$cli" "$*" >> "$CALL_LOG"
  if [ "${1:-}" = api ]; then
    endpoint="${2:-}"
    page="${endpoint##*page=}"
    case "$FIXTURE_SCENARIO" in
      direct_*) response="$FIXTURE_DIR/${FIXTURE_SCENARIO}_${page}.json" ;;
      *_invalid) response="$FIXTURE_DIR/invalid.json" ;;
      *_nonarray) response="$FIXTURE_DIR/object.json" ;;
      *) response="$FIXTURE_DIR/${cli}_files_${page}.json" ;;
    esac
    [ -f "$response" ] || { printf 'missing fixture: %s\n' "$response" >&2; return 90; }
    cat "$response"
    return $?
  fi
  case "$cli:$1:${2:-}" in
    gh:pr:list) cat "$FIXTURE_DIR/gh_list.json" ;;
    gh:pr:view) cat "$FIXTURE_DIR/gh_view.json" ;;
    glab:mr:list)
      page=1
      for arg in "$@"; do
        if [ "$previous" = --page ]; then page="$arg"; fi
        previous="$arg"
      done
      cat "$FIXTURE_DIR/gl_list_${page}.json" ;;
    glab:mr:view) cat "$FIXTURE_DIR/gl_view.json" ;;
    *) printf 'unexpected mocked forge call: %s %s\n' "$cli" "$*" >&2; return 91 ;;
  esac
}
if [ "$CALL_KIND" = pages ]; then
  forge_api_pages "$CLI" 'repos/fixture/project/files' 'github.com'
else
  forge_pr_metadata "$PLATFORM" 'fixture/project' 'feature/t4' 'development'
fi
'''


class ForgeAdapterFixture(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not ARCHIVE.is_file():
            raise RuntimeError('Pinned archive is missing; run scripts/test-harness.py')
        spec = importlib.util.spec_from_file_location(
            'forge_test_installer', PROJECT / 'scripts/install-harness.py'
        )
        cls.installer = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.installer)
        # Use the same checked, patched candidate set that the installer stages.
        candidates = cls.installer.prepare(ARCHIVE)
        cls.forge_bytes = candidates[PROJECT / '.codex/scripts/forge.sh']

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='spotify-forge-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fixtures = self.root / 'fixtures'
        self.fixtures.mkdir()
        self.forge_script = self.root / '.codex/scripts/forge.sh'
        self.forge_script.parent.mkdir(parents=True)
        self.forge_script.write_bytes(self.forge_bytes)
        self.call_log = self.root / 'calls.log'
        self.call_log.touch()

    def write_json(self, name, value):
        path = self.fixtures / name
        if isinstance(value, bytes):
            path.write_bytes(value)
        elif isinstance(value, str):
            path.write_text(value)
        else:
            path.write_text(json.dumps(value, separators=(',', ':')) + '\n')
        return path

    def call_forge(self, *, scenario, kind, cli='', platform=''):
        self.call_log.write_text('')
        env = dict(
            os.environ,
            FORGE_SCRIPT=str(self.forge_script),
            FIXTURE_DIR=str(self.fixtures),
            FIXTURE_SCENARIO=scenario,
            CALL_LOG=str(self.call_log),
            CALL_KIND=kind,
            CLI=cli,
            PLATFORM=platform,
        )
        return subprocess.run(
            ['bash', '-c', MOCK_FORGE], cwd=self.root, env=env,
            capture_output=True, text=True,
        )

    def large_names(self, prefix):
        return [
            f'{prefix}-{index:03d}-' + chr(97 + index % 26) * 2600 + '.txt'
            for index in range(101)
        ]

    def write_large_files(self, cli, names):
        key = 'filename' if cli == 'gh' else 'new_path'
        for page, items in ((1, names[:100]), (2, names[100:])):
            path = self.write_json(f'{cli}_files_{page}.json', [{key: name} for name in items])
            if page == 1:
                self.assertGreater(path.stat().st_size, 200 * 1024)

    def write_valid_metadata(self, platform):
        if platform == 'github':
            self.write_json('gh_list.json', [{
                'number': 42, 'headRefName': 'feature/t4', 'baseRefName': 'development',
                'state': 'OPEN', 'isCrossRepository': False,
            }])
            self.write_json('gh_view.json', {
                'number': 42, 'url': 'https://github.com/fixture/project/pull/42',
                'headRefName': 'feature/t4', 'baseRefName': 'development',
                'headRefOid': 'deadbeef', 'state': 'OPEN', 'isCrossRepository': False,
            })
            return

        self.write_json('gl_list_1.json', [{
            'iid': 1, 'source_branch': f'other-{index}', 'target_branch': 'development',
            'state': 'opened', 'source_project_id': 7, 'target_project_id': 7,
        } for index in range(100)])
        self.write_json('gl_list_2.json', [{
            'iid': 42, 'source_branch': 'feature/t4', 'target_branch': 'development',
            'state': 'opened', 'source_project_id': 7, 'target_project_id': 7,
        }])
        self.write_json('gl_view.json', {
            'iid': 42, 'web_url': 'https://gitlab.com/fixture/project/-/merge_requests/42',
            'source_branch': 'feature/t4', 'target_branch': 'development',
            'sha': 'facecafe', 'state': 'opened',
            'source_project_id': 7, 'target_project_id': 7,
        })

    def test_pages_stream_large_two_page_responses_and_stop_on_empty(self):
        names = self.large_names('api')
        self.write_json('direct_large_1.json', [{'filename': name} for name in names[:100]])
        self.write_json('direct_large_2.json', [{'filename': names[100]}])
        self.write_json('direct_empty_1.json', [])

        for cli in ('gh', 'glab'):
            with self.subTest(cli=cli):
                result = self.call_forge(scenario='direct_large', kind='pages', cli=cli)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual([item['filename'] for item in json.loads(result.stdout)], names)
                calls = self.call_log.read_text().splitlines()
                self.assertEqual(len(calls), 2)
                self.assertIn('page=1', calls[0])
                self.assertIn('page=2', calls[1])

                result = self.call_forge(scenario='direct_empty', kind='pages', cli=cli)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads(result.stdout), [])
                self.assertEqual(len(self.call_log.read_text().splitlines()), 1)

    def test_github_metadata_normalizes_and_retains_large_file_list(self):
        names = self.large_names('gh-file')
        self.write_large_files('gh', names)
        self.write_valid_metadata('github')

        result = self.call_forge(scenario='github_metadata', kind='metadata', platform='github')

        self.assertEqual(result.returncode, 0, result.stderr)
        metadata = json.loads(result.stdout)
        self.assertEqual(
            {key: metadata[key] for key in ('number', 'head', 'base', 'sha', 'state')},
            {'number': 42, 'head': 'feature/t4', 'base': 'development', 'sha': 'deadbeef', 'state': 'open'},
        )
        self.assertEqual(metadata['files'], names)
        api_calls = [line for line in self.call_log.read_text().splitlines() if line.startswith('gh api ')]
        self.assertEqual(len(api_calls), 2)
        self.assertIn('page=1', api_calls[0])
        self.assertIn('page=2', api_calls[1])

    def test_gitlab_metadata_normalizes_and_retains_large_file_list(self):
        names = self.large_names('gl-file')
        self.write_large_files('glab', names)
        self.write_valid_metadata('gitlab')

        result = self.call_forge(scenario='gitlab_metadata', kind='metadata', platform='gitlab')

        self.assertEqual(result.returncode, 0, result.stderr)
        metadata = json.loads(result.stdout)
        self.assertEqual(
            {key: metadata[key] for key in ('number', 'head', 'base', 'sha', 'state')},
            {'number': 42, 'head': 'feature/t4', 'base': 'development', 'sha': 'facecafe', 'state': 'open'},
        )
        self.assertEqual(metadata['files'], names)
        calls = self.call_log.read_text().splitlines()
        list_calls = [line for line in calls if line.startswith('glab mr list ')]
        api_calls = [line for line in calls if line.startswith('glab api ')]
        self.assertEqual(len(list_calls), 2)
        self.assertIn('--page 1', list_calls[0])
        self.assertIn('--page 2', list_calls[1])
        self.assertEqual(len(api_calls), 2)
        self.assertIn('page=1', api_calls[0])
        self.assertIn('page=2', api_calls[1])

    def test_malformed_and_non_array_responses_fail_for_both_platforms(self):
        self.write_json('direct_invalid_1.json', '{not json')
        self.write_json('direct_nonarray_1.json', {'filename': 'not-an-array'})
        self.write_json('invalid.json', '{not json')
        self.write_json('object.json', {'filename': 'not-an-array'})

        for cli in ('gh', 'glab'):
            for scenario in ('direct_invalid', 'direct_nonarray'):
                with self.subTest(cli=cli, scenario=scenario):
                    result = self.call_forge(scenario=scenario, kind='pages', cli=cli)
                    self.assertNotEqual(result.returncode, 0)

        for platform in ('github', 'gitlab'):
            self.write_valid_metadata(platform)
            for suffix in ('invalid', 'nonarray'):
                with self.subTest(platform=platform, response=suffix):
                    result = self.call_forge(
                        scenario=f'{platform}_{suffix}', kind='metadata', platform=platform,
                    )
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, '')


if __name__ == '__main__':
    unittest.main()
