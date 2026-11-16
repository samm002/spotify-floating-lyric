"""Isolated harness regression tests. No real worker/forge evidence is claimed."""
import copy
from datetime import datetime, timedelta, timezone
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SOURCE = Path(os.environ.get('HARNESS_SOURCE', Path(__file__).resolve().parents[2]))


class HarnessFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='spotify-lifecycle-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.configdir = self.root / '.codex'
        shutil.copytree(SOURCE / '.codex/scripts', self.configdir / 'scripts')
        shutil.copytree(SOURCE / '.codex/agents', self.configdir / 'agents')
        shutil.copy2(SOURCE / '.codex/goal-models.json', self.configdir / 'goal-models.json')
        # Lifecycle fixtures use controlled routes, independent of user budget choices.
        model_file = self.configdir / 'goal-models.json'
        fixture_models = json.loads(model_file.read_text())
        for levels in fixture_models['$routing'].values():
            for route in levels.values():
                if route is not None:
                    route.update(model='gpt-6.1-sol', model_reasoning_effort='medium')
        model_file.write_text(json.dumps(fixture_models))
        self.git('init', '-q', '-b', 'fixture')
        (self.root / '.git/info/exclude').write_text('.codex/\nstate.json\nforged.json\n')
        (self.root / 'README').write_text('fixture\n')
        self.git('add', 'README')
        self.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.test', '-c', 'commit.gpgsign=false', 'commit', '-qm', 'fixture')
        self.config = {'platform': 'github', 'concurrency': 1, 'worker_lifecycle': 'terminal_observation'}
        self.write_config()
        self.statepath = self.root / 'state.json'
        self.statepath.write_text(json.dumps([{
            'id': 'fixture-goal', 'run_id': 'fixture-run', 'agent_platform': 'codex',
            'goal': 'Synthetic lifecycle fixture', 'branch': 'fixture', 'base_branch': 'main',
            'status': 'in_progress', 'platform': 'github',
            'source': {'type': 'prompt', 'title': 'Fixture', 'body': 'Fixture only', 'reference': '', 'acceptance_criteria': []},
            'repos': [],
        }]))
        script = (self.configdir / 'scripts/goal-git.sh').read_text().rsplit('\ncase "${1:-}" in\n', 1)[0]
        script += '''
case "${1:-}" in
  role-completed) shift; harness_require; harness_role_completed "$@" ;;
  evidence-role) shift; harness_require; harness_completed_role "$@" ;;
  impl-ready) harness_require; harness_impl_tasks_ready ;;
  done-fixture) harness_require_current_evidence() { :; }; cmd_harness_done ;;
  reap-race-fixture)
    harness_now() {
      bash "$SCRIPTS_DIR/goal-git.sh" harness spawn builder gpt-6.1-sol medium --task t1 > "$PROJECT_ROOT/.codex/race-reservation.json" || return
      local reserved worker
      reserved=$(jq -r .reservation.id "$PROJECT_ROOT/.codex/race-reservation.json")
      worker=$(jq -r .reservation.worker_task_name "$PROJECT_ROOT/.codex/race-reservation.json")
      bash "$SCRIPTS_DIR/goal-git.sh" harness spawn-confirm "$reserved" "/root/$worker" >/dev/null || return
      date -Iseconds
    }
    harness_require; cmd_harness_reap ;;
  merge-flag) shift; merge_flag_for "$@" ;;
  merge-test)
    delivery_clean() { :; }; context_assert_branch() { :; }
    delivery_repo() { printf '%s' https://github.com/fixture/repo; }
    delivery_remote_request() { printf '{"number":1}'; }
    delivery_merged_request_ok() { [ -f "$PROJECT_ROOT/forged.json" ]; }
    cmd_pending() { :; }; cmd_review_pending() { :; }
    forge_pr_metadata() { printf '{"number":1}'; }; delivery_validate_metadata() { :; }
    forge_require_flags() { :; }
    forge_run() { python3 - "$PROJECT_ROOT/forged.json" "$@" <<'ARGS'
import json,sys
open(sys.argv[1], 'w').write(json.dumps(sys.argv[2:]))
ARGS
    }
    refresh_goal_idx; merge_pr_in_dir "$PROJECT_ROOT" 1 https://github.com/fixture/repo/pull/1 ;;
  harness) shift; cmd_harness "$@" ;;
  *) exit 98 ;;
esac
'''
        self.runner = self.configdir / 'scripts/fixture.sh'
        self.runner.write_text(script)
        self.env = dict(os.environ, GOAL_PLATFORM='github', GOAL_ID='', GOAL_RUN_ID='', GOAL_ISSUE='', GOAL_GROUP='', GOAL_TASK='', GOAL_REPO='', GOAL_ISSUE_REPO='')
        self.run_cmd('harness', 'init', '--complexity', 'NORMAL', '--route', 'feature', '--visual', 'false')
        self.edit(lambda g: g['harness'].update(phase='BUILDING'))

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.root), *args], check=True, capture_output=True, text=True).stdout.strip()

    def write_config(self):
        self.configdir.mkdir(parents=True, exist_ok=True)
        (self.configdir / 'goal-config.json').write_text(json.dumps(self.config))

    def state(self):
        return json.loads(self.statepath.read_text())[0]

    def edit(self, func):
        state = self.state()
        func(state)
        self.statepath.write_text(json.dumps([state]))

    def run_cmd(self, *args, ok=True):
        result = subprocess.run(['bash', str(self.runner), *args], cwd=self.root, env=self.env, capture_output=True, text=True)
        if ok:
            self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        else:
            self.assertNotEqual(result.returncode, 0, result.stderr + result.stdout)
        return result

    def reserve(self, role='builder', task=True):
        args = []
        if task:
            self.run_cmd('harness', 'task', 'add', role, 'Fixture work')
            taskid = self.state()['harness']['tasks'][-1]['id']
            args = ['--task', taskid]
        reservation = json.loads(self.run_cmd('harness', 'spawn', role, 'gpt-6.1-sol', 'medium', *args).stdout)['reservation']
        return reservation

    def confirm(self, reservation):
        agent = '/root/' + (reservation.get('worker_task_name') or 'native_fixture')
        self.run_cmd('harness', 'spawn-confirm', reservation['id'], agent)
        return agent

    def observation(self, reservation, agent):
        return {
            'source': 'collaboration.list_agents', 'observed_at': datetime.now(timezone.utc).isoformat(timespec='seconds'),
            'reservation_id': reservation['id'], 'goal_key': reservation['goal_key'], 'run_id': reservation['run_id'],
            'group_id': reservation['group_id'], 'task_id': reservation['task'], 'rework_cycle': reservation['rework_cycle'],
            'sha': reservation['sha'], 'agent_name': agent,
            'snapshot': {'agents': [{'agent_name': agent, 'agent_status': {'completed': 'Synthetic test worker result'}}]},
        }

    def finish(self, reservation, agent, outcome='completed', observation=None, ok=True):
        data = observation if observation is not None else self.observation(reservation, agent)
        path = self.configdir / 'observation.json'
        path.write_text(json.dumps(data))
        return self.run_cmd('harness', 'spawn-finish', agent, outcome, '--observation-file', str(path), ok=ok)

    def test_observation_releases_capacity_without_closure(self):
        r = self.reserve(); agent = self.confirm(r)
        self.config['worker_lifecycle'] = 'native_close'; self.write_config()
        self.finish(r, agent)
        saved = self.state()['spawn_reservations'][0]
        self.assertFalse(saved['closed'])
        self.assertEqual(saved['terminal_observation']['snapshot']['agents'][0]['agent_name'], agent)
        self.run_cmd('impl-ready')
        self.run_cmd('harness', 'task', 'set', r['task'], 'DONE')
        self.config['worker_lifecycle'] = 'terminal_observation'; self.write_config()
        r2 = self.reserve()
        self.assertNotEqual(r['worker_task_name'], r2['worker_task_name'])
        self.run_cmd('harness', 'spawn-confirm', r2['id'], agent, ok=False)

    def test_native_default_and_frozen_mode(self):
        self.config.pop('worker_lifecycle'); self.write_config()
        r = self.reserve(); agent = self.confirm(r)
        self.assertEqual(r['worker_lifecycle'], 'native_close')
        self.edit(lambda g: [r.pop('worker_lifecycle', None) for r in g['spawn_reservations']])
        self.config['worker_lifecycle'] = 'terminal_observation'; self.write_config()
        self.finish(r, agent, ok=False)
        self.run_cmd('harness', 'spawn-finish', agent, 'completed')
        self.run_cmd('impl-ready', ok=False)
        self.run_cmd('harness', 'task', 'set', r['task'], 'DONE', ok=False)
        self.run_cmd('harness', 'spawn-finish', agent, 'completed', '--closed')
        self.run_cmd('impl-ready')
        self.assertTrue(self.state()['spawn_reservations'][0]['closed'])

    def test_observer_cannot_assert_close_and_unknown_mode_rejected(self):
        r = self.reserve(); agent = self.confirm(r)
        before = self.statepath.read_bytes()
        self.run_cmd('harness', 'spawn-finish', agent, 'completed', '--closed', ok=False)
        self.assertEqual(before, self.statepath.read_bytes())
        self.config['worker_lifecycle'] = 'invalid'; self.write_config()
        self.run_cmd('harness', 'spawn', 'researcher', 'gpt-6.1-sol', 'medium', ok=False)

    def test_identity_status_and_time_rejection_is_atomic(self):
        r = self.reserve(); agent = self.confirm(r)
        base = self.observation(r, agent)
        invalid = []
        for key in ('source', 'reservation_id', 'goal_key', 'run_id', 'group_id', 'task_id', 'rework_cycle', 'sha', 'agent_name'):
            o = copy.deepcopy(base); o[key] = 'mismatch'; invalid.append((key, o))
            o = copy.deepcopy(base); del o[key]; invalid.append(('missing ' + key, o))
        for delta in (-86400, 86400):
            o=copy.deepcopy(base); o['observed_at']=(datetime.now(timezone.utc)+timedelta(seconds=delta)).isoformat(timespec='seconds'); invalid.append(('time',o))
        for status in ('running', 'idle', {}, {'completed': ''}, {'errored': 'unknown status'}, {'completed': 'x', 'running': True}):
            o=copy.deepcopy(base); o['snapshot']['agents'][0]['agent_status']=status; invalid.append(('status',o))
        for agents in ([], [base['snapshot']['agents'][0]]*2):
            o=copy.deepcopy(base); o['snapshot']['agents']=agents; invalid.append(('agents',o))
        for label,o in invalid:
            with self.subTest(label=label):
                before = self.statepath.read_bytes()
                self.finish(r,agent,observation=o,ok=False)
                self.assertEqual(before,self.statepath.read_bytes())
        self.finish(r,agent,observation=base)
        self.finish(r,agent,observation=base)  # exact repeat is idempotent
        changed=copy.deepcopy(base); changed['snapshot']['agents'][0]['agent_status']['completed']='different result'
        self.finish(r,agent,observation=changed,ok=False)
        path = self.configdir / 'malformed.json'; path.write_text('{invalid')
        before = self.statepath.read_bytes()
        self.run_cmd('harness','spawn-finish',agent,'completed','--observation-file',str(path),ok=False)
        self.assertEqual(before,self.statepath.read_bytes())

    def test_recovery_never_releases_running_observer(self):
        r=self.reserve(); agent=self.confirm(r)
        for args in [('harness','reap'),('harness','task','retry',r['task']),('harness','task','cancel',r['task'])]:
            before=self.statepath.read_bytes(); self.run_cmd(*args,ok=False); self.assertEqual(before,self.statepath.read_bytes())
        self.finish(r,agent,outcome='failed')
        self.run_cmd('impl-ready',ok=False)
        self.assertFalse(self.state()['spawn_reservations'][0]['closed'])
        self.run_cmd('harness','task','retry',r['task'])
        self.assertEqual(self.state()['harness']['tasks'][0]['state'],'PENDING')
        r2=json.loads(self.run_cmd('harness','spawn','builder','gpt-6.1-sol','medium','--task',r['task']).stdout)['reservation']
        self.assertNotEqual(r['worker_task_name'],r2['worker_task_name'])
        agent2=self.confirm(r2)
        self.finish(r2,agent2,observation=self.observation(r,agent),ok=False)

    def test_observer_appearing_during_reap_is_not_falsely_closed(self):
        self.run_cmd('harness','task','add','builder','Race fixture')
        self.run_cmd('reap-race-fixture',ok=False)
        record=self.state()['spawn_reservations'][0]
        self.assertEqual(record['state'],'confirmed')
        self.assertFalse(record['closed'])
        self.assertNotIn('terminal_observation',record)
        self.assertEqual(self.state()['harness']['tasks'][0]['state'],'RUNNING')

    def test_definitive_unlaunched_failure_and_uncertain_hold(self):
        r=self.reserve()
        self.run_cmd('harness','spawn-fail',r['id'],'uncertain','Interrupted launch')
        self.run_cmd('harness','reap',ok=False)
        self.assertFalse(self.state()['spawn_reservations'][0]['closed'])
        self.run_cmd('harness','spawn-fail',r['id'],'capability','Definitely no worker launched')
        self.assertFalse(self.state()['spawn_reservations'][0]['closed'])
        self.run_cmd('harness','task','retry',r['task'])
        self.run_cmd('harness','spawn','builder','gpt-6.1-sol','medium','--task',r['task'])

    def test_review_evidence_rejects_rework_and_new_commit(self):
        self.edit(lambda g:g['harness'].update(phase='REVIEWING'))
        r=self.reserve('reviewer',task=False); agent=self.confirm(r); self.finish(r,agent)
        self.run_cmd('role-completed','reviewer'); self.run_cmd('evidence-role','reviewer')
        self.edit(lambda g:g['harness']['counters'].update(rework=1))
        self.run_cmd('role-completed','reviewer',ok=False); self.run_cmd('evidence-role','reviewer',ok=False)
        self.edit(lambda g:g['harness']['counters'].update(rework=0))
        (self.root/'README').write_text('new implementation\n'); self.git('add','README')
        self.git('-c','user.name=Fixture','-c','user.email=fixture@example.test','-c','commit.gpgsign=false','commit','-qm','change')
        self.run_cmd('role-completed','reviewer',ok=False); self.run_cmd('evidence-role','reviewer',ok=False)

    def test_semantic_review_failure_cannot_supply_success(self):
        self.edit(lambda g:g['harness'].update(phase='REVIEWING'))
        r=self.reserve('reviewer',task=False); agent=self.confirm(r); self.finish(r,agent,outcome='failed')
        self.run_cmd('role-completed','reviewer',ok=False); self.run_cmd('evidence-role','reviewer',ok=False)

    def test_rework_retains_old_done_tasks_and_requires_fresh_builder(self):
        original=self.reserve(); agent=self.confirm(original); self.finish(original,agent)
        self.run_cmd('impl-ready')
        self.edit(lambda g:g['harness']['counters'].update(rework=1))
        self.run_cmd('impl-ready',ok=False)
        fresh=self.reserve(); fresh_agent=self.confirm(fresh); self.finish(fresh,fresh_agent)
        self.run_cmd('impl-ready')
        self.assertTrue(all(t['state']=='DONE' for t in self.state()['harness']['tasks']))
        self.assertEqual(self.state()['spawn_reservations'][0]['rework_cycle'],0)
        self.assertEqual(self.state()['spawn_reservations'][1]['rework_cycle'],1)

    def seed_terminal_gate_fixture(self):
        # Synthetic prerequisites isolate the worker-release barrier only.
        def seed(g):
            g['harness']['phase']='DONE'
            g['harness']['requirements']={'planner':False,'reviewer':False,'visual':False}
            g['harness']['gates']={name:{'status':'PASS'} for name in ['IMPLEMENTATION','ANALYSIS','VERIFICATION']}
            for task in g['harness']['tasks']: task['state']='CANCELLED'
        self.edit(seed)

    def test_terminal_gate_requires_release_even_for_failed_native(self):
        self.config['worker_lifecycle']='native_close'; self.write_config()
        r=self.reserve(); agent=self.confirm(r)
        self.run_cmd('harness','spawn-finish',agent,'failed')
        self.seed_terminal_gate_fixture()
        self.run_cmd('done-fixture',ok=False)
        self.run_cmd('harness','spawn-finish',agent,'failed','--closed')
        self.run_cmd('done-fixture')

    def test_terminal_gate_accepts_observed_release_without_closure(self):
        r=self.reserve(); agent=self.confirm(r)
        self.seed_terminal_gate_fixture()
        self.run_cmd('done-fixture',ok=False)
        self.finish(r,agent)
        self.run_cmd('done-fixture')
        self.assertFalse(self.state()['spawn_reservations'][0]['closed'])

    def test_branch_policy_selects_actual_forge_arguments(self):
        self.config.update(review_mode='local',branch_flow={'integration_branch':'development','release_branch':'main'}); self.write_config()
        self.git('checkout','-qb','feat/demo')
        self.edit(lambda g:g.update(branch='feat/demo',base_branch='development',pr_number=1))
        self.run_cmd('merge-test')
        self.assertIn('--squash',json.loads((self.root/'forged.json').read_text()))
        (self.root/'forged.json').unlink()
        self.git('checkout','-qb','development')
        self.edit(lambda g:g.update(branch='development',base_branch='main'))
        self.run_cmd('merge-test')
        args=json.loads((self.root/'forged.json').read_text()); self.assertIn('--merge',args); self.assertNotIn('--squash',args)
        self.run_cmd('merge-flag','feat/direct','main',ok=False)
        self.config.pop('branch_flow'); self.write_config()
        self.assertEqual(self.run_cmd('merge-flag','feat/legacy','main').stdout.strip(),'--merge')


if __name__ == '__main__':
    unittest.main()
