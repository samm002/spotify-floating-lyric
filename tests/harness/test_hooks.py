"""Synthetic script-level hook behavior checks; no client hook firing is claimed."""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SOURCE = Path(os.environ.get('HARNESS_SOURCE', Path(__file__).resolve().parents[2]))


class HookBehaviorFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='spotify-hook-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.configdir = self.root / '.codex'
        shutil.copytree(SOURCE / '.codex/scripts', self.configdir / 'scripts')
        shutil.copytree(SOURCE / '.codex/agents', self.configdir / 'agents')
        shutil.copy2(SOURCE / '.codex/goal-models.json', self.configdir / 'goal-models.json')
        model_file = self.configdir / 'goal-models.json'
        fixture_models = json.loads(model_file.read_text())
        for levels in fixture_models['$routing'].values():
            for route in levels.values():
                if route is not None:
                    route.update(model='gpt-6.1-sol', model_reasoning_effort='medium')
        model_file.write_text(json.dumps(fixture_models))
        self.git('init', '-q', '-b', 'fixture')
        (self.root / '.git/info/exclude').write_text('.codex/\nstate.json\n')
        (self.root / 'README').write_text('fixture\n')
        self.git('add', 'README')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')
        self.config = {
            'platform': 'github',
            'concurrency': 1,
            'worker_lifecycle': 'terminal_observation',
            'stop_check_window': 180,
        }
        self.write_config()
        self.statepath = self.root / 'state.json'
        self.statepath.write_text(json.dumps([{
            'id': 'fixture-goal', 'run_id': 'fixture-run', 'agent_platform': 'codex',
            'goal': 'Synthetic hook fixture', 'branch': 'fixture', 'base_branch': 'main',
            'status': 'in_progress', 'platform': 'github',
            'source': {'type': 'prompt', 'title': 'Fixture', 'body': 'Fixture only', 'reference': '', 'acceptance_criteria': []},
            'repos': [],
        }]))
        self.helper = self.configdir / 'scripts/goal-git.sh'
        self.env = dict(os.environ, GOAL_PLATFORM='github', GOAL_ID='', GOAL_RUN_ID='', GOAL_ISSUE='', GOAL_GROUP='', GOAL_TASK='', GOAL_REPO='', GOAL_ISSUE_REPO='')
        self.run_cmd('harness', 'init', '--complexity', 'NORMAL', '--route', 'feature', '--visual', 'false')
        self.edit(lambda goal: goal['harness'].update(phase='BUILDING'))
        recent = datetime.now(timezone.utc).isoformat(timespec='seconds')
        self.edit(lambda goal: goal['harness'].update(events=[{'at': recent, 'agent': 'MAIN', 'event': 'activity', 'detail': 'Synthetic recent activity'}]))

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.root), *args], check=True, capture_output=True, text=True).stdout.strip()

    def write_config(self):
        self.configdir.mkdir(parents=True, exist_ok=True)
        (self.configdir / 'goal-config.json').write_text(json.dumps(self.config))

    def state(self):
        return json.loads(self.statepath.read_text())[0]

    def edit(self, update):
        goals = json.loads(self.statepath.read_text())
        update(goals[0])
        self.statepath.write_text(json.dumps(goals))

    def run_cmd(self, *args, input_text=None, ok=True):
        result = subprocess.run(
            ['bash', str(self.helper), *args], cwd=self.root, env=self.env,
            input=input_text, capture_output=True, text=True,
        )
        if ok:
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        else:
            self.assertNotEqual(result.returncode, 0, result.stderr + result.stdout)
        return result

    def confirmed_reservation(self):
        self.run_cmd('harness', 'task', 'add', 'builder', 'Synthetic hook work')
        task = self.state()['harness']['tasks'][-1]['id']
        reservation = json.loads(self.run_cmd('harness', 'spawn', 'builder', 'gpt-6.1-sol', 'medium', '--task', task).stdout)['reservation']
        agent = '/synthetic/' + (reservation.get('worker_task_name') or 'hook-fixture')
        self.run_cmd('harness', 'spawn-confirm', reservation['id'], agent)
        return reservation, agent

    def fire(self, event, agent_id, **payload):
        data = {'hook_event_name': event, 'agent_type': 'builder', 'agent_id': agent_id, **payload}
        return self.run_cmd('harness', 'hook', input_text=json.dumps(data))

    def stop(self):
        return self.run_cmd('harness', 'stop-check')

    def test_subagent_hooks_append_informational_events_only(self):
        reservation, agent = self.confirmed_reservation()
        before = self.state()['harness']
        start = self.fire('SubagentStart', agent)
        self.assertEqual(json.loads(start.stdout), {'continue': True})
        stopped = self.fire('SubagentStop', agent, last_assistant_message='Synthetic worker finished')
        self.assertTrue(json.loads(stopped.stdout)['continue'])

        after = self.state()['harness']
        self.assertEqual([event['event'] for event in after['events'][-2:]], ['started', 'completed'])
        self.assertEqual([event['agent'] for event in after['events'][-2:]], ['builder', 'builder'])
        self.assertEqual(after['phase'], before['phase'])
        self.assertEqual(after['gates'], before['gates'])
        self.assertFalse(any(gate.get('status') == 'PASS' for gate in after['gates'].values()))
        saved = after['spawn_reservations'][0]
        self.assertEqual(saved['id'], reservation['id'])
        self.assertEqual(saved['state'], 'confirmed')
        self.assertFalse(saved['closed'])
        self.assertEqual(after['tasks'][0]['state'], 'RUNNING')

    def test_stop_blocks_unfinished_recent_goal_with_next_action(self):
        result = self.stop()
        decision = json.loads(result.stdout)
        self.assertEqual(decision['decision'], 'block')
        self.assertIn('Next:', decision['reason'])

    def test_stop_allows_saved_blocker_and_done_phase(self):
        self.run_cmd('harness', 'event', 'blocker', 'blocker', 'Synthetic user blocker')
        self.assertEqual(self.stop().stdout, '')

        self.edit(lambda goal: goal['harness'].update(phase='DONE', events=[event for event in goal['harness']['events'] if event['event'] != 'blocker']))
        self.assertEqual(self.stop().stdout, '')

    def test_stop_fails_open_without_state(self):
        self.statepath.unlink()
        result = self.stop()
        self.assertEqual(result.stdout, '')
        self.assertEqual(result.returncode, 0)

    def test_stop_allows_fourth_unchanged_state_attempt(self):
        initial = self.statepath.read_bytes()
        for _ in range(3):
            self.assertEqual(json.loads(self.stop().stdout)['decision'], 'block')
        self.assertEqual(self.stop().stdout, '')
        self.assertEqual(self.statepath.read_bytes(), initial)
        guard = json.loads((self.configdir / 'stop-guard.json').read_text())
        self.assertEqual(guard['count'], 3)


if __name__ == '__main__':
    unittest.main()
