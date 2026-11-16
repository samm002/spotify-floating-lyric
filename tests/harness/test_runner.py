"""Verify shared-root fixture discovery without recursively invoking the test suite."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

PROJECT = Path(__file__).resolve().parents[2]


class RunnerFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='spotify-runner-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'workflow'; self.root.mkdir()
        (self.root / 'scripts').mkdir(); (self.root / 'harness').mkdir()
        shutil.copy2(PROJECT / 'scripts/test-harness.py', self.root / 'scripts/test-harness.py')
        payload = b'Synthetic cache fixture, not upstream execution evidence'
        digest = hashlib.sha256(payload).hexdigest()
        (self.root / 'harness/upstream.lock.json').write_text(json.dumps({'archive_sha256': digest}))
        helper = self.root / '.codex/scripts/goal-git.sh'; helper.parent.mkdir(parents=True)
        helper.write_text('# Synthetic path fixture; never executed\n')
        self.archive = self.root / '.tools/cache' / f'upstream-{digest}.tar.gz'
        self.archive.parent.mkdir(parents=True); self.archive.write_bytes(payload)
        (self.root / '.gitignore').write_text('.codex/\n.tools/\n')
        self.git('init','-q','-b','fixture'); self.git('add','scripts','harness','.gitignore')
        self.git('-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgsign=false','commit','-qm','fixture')
        self.checkout = Path(self.temp.name) / 'worktree'
        self.git('worktree','add','--detach',str(self.checkout),'HEAD')
        self.env = dict(os.environ); self.env.pop('GOAL_GIT',None)

    def git(self,*args):
        subprocess.run(['git','-C',str(self.root),*args],check=True,capture_output=True)

    def run_wrapper(self,*args):
        return subprocess.run([sys.executable,str(self.checkout/'scripts/test-harness.py'),*args],cwd=self.checkout,env=self.env,capture_output=True,text=True)

    def test_worktree_uses_shared_workflow_and_archive(self):
        self.assertFalse((self.checkout/'.codex').exists())
        result=self.run_wrapper('--check'); self.assertEqual(result.returncode,0,result.stderr)
        resolved=json.loads(result.stdout)
        self.assertEqual(resolved['HARNESS_SOURCE'],str(self.root))
        self.assertEqual(resolved['HARNESS_ARCHIVE'],str(self.archive))
        self.env['GOAL_GIT']=str(self.root/'.codex/scripts/goal-git.sh')
        result=self.run_wrapper('--check'); self.assertEqual(result.returncode,0,result.stderr)
        self.assertEqual(json.loads(result.stdout),resolved)

    def test_missing_cache_blocks_before_tests(self):
        self.archive.unlink()
        result=self.run_wrapper()
        self.assertEqual(result.returncode,2)
        self.assertIn('BLOCKED:',result.stderr)
        self.assertNotIn('skipped',result.stdout+result.stderr)

    def test_corrupt_cache_blocks_before_tests(self):
        self.archive.write_bytes(b'corrupt')
        result=self.run_wrapper()
        self.assertEqual(result.returncode,2)
        self.assertIn('checksum mismatch',result.stderr)


if __name__=='__main__':
    unittest.main()
