# Setup finalization — 2026-11-14

## Boundary and assignments

This is bootstrap maintenance on `chore/harness-bootstrap`, not an upstream
application goal, gate pass, PR delivery, or merge. Application development
remains paused. Existing related changes to AGENTS.md, model policy, installer,
and lifecycle fixture isolation were preserved. LICENSE and application source
were unchanged. No application `state.json` exists.

- Read-only Planner: `/root/bootstrap_plan_20261114_a41c9f`, Sol/medium.
- Builder: `/root/bootstrap_build_20261114_f7d208`, Luna/xhigh.
- MAIN reconciled two generated runtime files, ran verification and live client
  probes, recorded evidence, and prepared the local checkpoint.
- Independent Reviewer: `/root/bootstrap_review_20261114_d8e301`, Sol/medium.
  Reviewed checkpoint `d965a46ecd624797afc0476a4d9725a937821095` against
  `da2229a`; returned **LGTM with no actionable findings**. This entry records
  the actual outcome after that committed-source review.

These were fresh native bootstrap assignments with focused role/ownership
briefs and `fork_turns: "none"`. They were not upstream goal reservations.
No separate worker-close operation is claimed.

The Reviewer inspected all eight changed files, the clean worktree, QA log,
smoke and approval summaries, and raw helper traces. Both recorded smoke hashes
matched. The Reviewer did not rerun the verification suite or inspect private
user configuration directly. This follow-up changes only the review record;
it does not claim another review of a different implementation commit.

## Reproducible configuration

`harness/upstream.lock.json` now preserves the user's Visual Reviewer choice
of `gpt-6.1-sol`/`low` for every complexity. It also sets `hooks_enabled: true`;
the installer now honors that boolean and generates worktree-safe hook commands.
The existing MAIN Sol/high, Builder Luna/xhigh, and Researcher Sol/low choices
remain intact. The source lock is required because runtime `.codex` files are
ignored and runtime-only customization is deliberately rejected by reinstall QA.

Only `.codex/config.toml` and `.codex/hooks.json` required reconciliation with
the prepared output. Other installed files already matched. The installer
retains archive/patch validation and refusal to overwrite unexpected changes.
Hook commands use Git's absolute shared common directory to find the root
helper from subdirectories and linked worktrees; no helper copy is needed there.

## Executed checks

- `PATH="$PWD/.tools/bin:$PATH" python3 scripts/test-harness.py`: **30 passed,
  zero skipped**, including the previously failing install-reproduction check.
  The run used `PYTHONDONTWRITEBYTECODE=1`; its log is
  `.tools/harness-final-qa.log`.
- `python3 scripts/install-harness.py <verified cached archive> --check`:
  **33 planned template files validated**, no files written.
- Runtime policy and verification commands matched `harness/project-policy.json`
  and `harness/verify-commands.json`; no application goal state was created.
- `PATH="$PWD/.tools/bin:$PATH" .codex/scripts/goal-git.sh doctor --json`:
  `ready=true`, `analyze_ready=true`, `invalid_roles=[]`; GitHub authenticated.
  This is discovery/readiness evidence, not an actual GitNexus analysis run.
- `bash scripts/verify.sh`: formatting, locked build, Clippy with warnings denied,
  and Rust test command passed. The app has **zero Rust unit tests**.
  The first sandboxed attempt hit DNS restrictions; the approved network-enabled
  retry succeeded. No desktop behavior was retested.
- `bash -n`: all nine installed/project shell scripts passed.
- `git diff --check`: passed before checkpointing.

Added script tests use isolated synthetic state. They establish that lifecycle
hooks log informational events without releasing reservations or passing gates;
Stop requests continuation for a recent unfinished goal, permits saved blockers
and DONE, fails open without state, and permits the fourth unchanged-state stop.
Installer tests cover enabled/disabled hooks, model/permission policy, and helper
resolution from subdirectories and linked worktrees with spaces in their paths.

## Hook approval and actual client smoke

Codex CLI/app-server **0.169.0** recognized the three project hooks. The user
explicitly approved enabling/testing them, then a narrow exception to AGENTS.md's
no-global-configuration rule for only these repository-specific trust records.
MAIN used the supported CLI hook review interface; no hook-trust bypass or
approval/sandbox override was used. Exactly three approval entries were added
to the user's configuration. A semantic fingerprint comparison confirmed that
all unrelated settings and other hook approvals remained unchanged.

Official behavior references:
[hook trust review](https://learn.chatgpt.com/docs/hooks#review-and-trust-hooks),
[CLI trust persistence](https://github.com/openai/codex/blob/main/codex-rs/tui/src/hooks_rpc.rs).

The ignored `.tools/hook-client-probe.py` used the installed app-server protocol,
with an ephemeral thread, on-request approval, and workspace-write sandboxing.
The first smoke returned real worker and parent markers. Its asynchronous Start
callback was absent from the protocol stream, so a second fresh smoke recorded
filtered process execution evidence rather than treating the missing callback
as a failure or fabricating it.

Traced parent thread: `01a1d871-caf8-7a83-aaaf-45ae863011dc`.
Its one actual read-only worker was
`/root/hook_smoke_8cb4ae0cbe7c41f790514231eb09e6f7`, thread
`01a1d871-e6d3-71d0-9214-9ad7dbef1fe0`, Sol/low. It returned
`HOOK_SMOKE_WORKER_OK`; the parent returned `HOOK_SMOKE_MAIN_OK`.

The process trace showed three actual helper invocations and successful exits:

| Hook | Evidence | Helper PID | Exit |
| --- | --- | --- | --- |
| SubagentStart | First `harness hook`, while the worker was starting | 111794 | 0 |
| SubagentStop | Second `harness hook`, plus completed client callback | 111893 | 0 |
| Stop | `harness stop-check`, plus completed client callback | 112034 | 0 |

The async-start identification follows the registered event commands and their
observed ordering. The protocol supplied successful completion callbacks for
SubagentStop and Stop. The raw trace separately proves the first helper process
executed and exited successfully; it does not claim a missing Start callback.

Local, ignored artifacts:

- `.tools/hook-probe/smoke-traced.jsonl`, SHA256
  `5a54b9a994a7cf65a36ea6461b863fd2e15bf06295b25cd68afded2d65d4657f`.
- `.tools/hook-probe/smoke-traced.stderr.log`, SHA256
  `bf0943bd10877b17e923758254ec8cc6fe1cefa3592952b9ee90167dbcebe353`.
- `.tools/hook-probe/smoke-summary.json` records helper times, PIDs, callbacks,
  worker activity, and absence of application goal state.
- `.tools/hook-probe/user-config-check.json` records the narrow approval result.

The live smoke exercised real hook dispatch and safe no-goal behavior. Active-goal
logging/continuation were tested only in synthetic script fixtures. No native
closure or full application-quality workflow completion is claimed. Fresh
clients or modified hook definitions still require their supported trust review.
