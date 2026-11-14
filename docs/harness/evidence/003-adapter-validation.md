# Adapter validation and project configuration — 2026-11-13

## Result

Installed the reviewed, checksum-bound adapter and configured the project.
This completes lifecycle compatibility setup. It is not a completed application
goal, committed-SHA harness gate, analysis run, PR verdict, or merge.

## Automated and independent checks

- Builder: 23 isolated unittest methods passed, zero skipped.
- Independent Reviewer: the same 23 methods passed against staged helpers and
  the pinned archive; no unresolved actionable findings.
- Installed project: `python3 scripts/test-harness.py` passed all 23 methods,
  zero skipped, using the actual shared installation and verified archive cache.
- Upstream native-mode delegation and stall-recovery suites passed against the
  patched helper tree, retaining default close-based behavior.
- Both revised skill entrypoints passed the skill-creator validator. A separate
  read-only forward test checked realistic launch, completion and recovery use.
- Shell syntax validation passed. Installer preparation matched all 33 staged
  files exactly; post-install `--check` reproduced the installed bytes.
- Runtime configuration was compared to the tracked policy and verification
  profile; all selected fields matched. No application `state.json` exists.

Coverage includes frozen lifecycle modes, observation identity/status/time
rejection, concurrency release, semantic failure, rework and commit isolation,
recovery restrictions, merge flags, installer checksums/refusals, and worktree QA.

## Findings corrected before installation

Independent review reproduced a recovery race: an observer created after an
unlocked precheck could be reaped with false closure. The final code revalidates
under the mutation lock and filters release to native reservations. The exact
interleaving is now a regression test.

Other corrected issues were stale recovery/launch instructions and formal QA
looking for `.codex` in an isolated worktree. The test runner now finds the shared
workflow root, requires the checksum-verified archive cache, and blocks missing
or corrupt fixtures instead of skipping installer tests. Rework retains prior
DONE-task evidence while requiring a fresh successful Builder for the new cycle.

## Real worker smoke

In a separate temporary test repository, MAIN reserved and confirmed a real
Researcher with the generated unique task name. The actual collaboration worker
`/root/worker_spawn_3cda6976_9fd2_4951_bc23_a1e6e284146f` returned
`LIVE_TERMINAL_SMOKE_OK`. MAIN collected its real `collaboration.list_agents`
snapshot and supplied the matching observation envelope to the helper.

The helper accepted and retained the observation, recorded state `completed`,
and kept `closed=false`. A fresh unique reservation could then be made under
one-worker concurrency; that second reservation was conclusively marked not
launched, with no second worker tool call and no closure claim. Both saved
reservations retained `closed=false`.

The live smoke's core helper bytes were compared with the final staged helpers
and matched. This proves the tested execution-receipt path, not native closure
or completion of the fixture's application-quality gates. Synthetic fixture
records are not substituted for actual worker observations.

## Saved user policy

`harness/project-policy.json` and `harness/verify-commands.json` reproduce the
ignored runtime configuration through `scripts/configure-harness.py`:

- Markdown default, with explicit GitHub issue runs on this repository.
- `development` target, per-change branches, inline PR review.
- Squash feature PRs into development; normal merge development into main.
  Other heads targeting main are rejected by the opt-in helper policy.
- One active worker; issue batches initially three, with further batches chosen
  as needed for the project goal.
- No automatic merging or Figma integration. Hooks remain disabled and their
  execution is not claimed.
- Observation lifecycle explicitly enabled; the native lifecycle stays default
  for installations/reservations that do not opt in.
- Formal QA includes formatting, locked build, Clippy, Rust tests and the
  worktree-safe harness regression runner.

The installer cached the verified upstream archive under ignored `.tools/cache`.
No global trust, permission, or authentication configuration was changed.
Only the remote development base branch was created; implementation source has
not been published and no PR or merge was performed during this setup.

## Next boundary

Checkpoint the reviewed bootstrap source before starting the first actual
application goal. Existing mock-window code remains the already verified local
pilot. Use a new source snapshot and fresh role evidence for a subsequent full
goal; do not retroactively label pilot work as a gated run.
